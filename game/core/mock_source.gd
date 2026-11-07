# mock_source.gd — simulación de JDSource (contracts/gdextension_api.md) para cuando la GDExtension
# no está cargada (B2/B9, agente D). Misma interfaz que la clase nativa; resultados pseudoaleatorios
# con semilla (reproducibles). Lo crea normalmente game/core/engine_factory.gd.
#
# Tipos de fuente:
#   - servidor de móviles: un "móvil" falso "Pixel (simulado)" se conecta `connect_delay_s` después de
#     start() y emite muestras a 200 Hz (gravedad + balanceo suave + ruido). Para probar la calibración
#     sin móvil: simulate_rhythm() / inject_shake() añaden sacudidas (|a| ≈ 31 m/s²).
#   - reproducción: lee una sesión .jsonl (contracts/session_format.md) y la emite en tiempo real.
#   - grabador: envuelve otra fuente y escribe lo que pasa por poll_samples() en una sesión .jsonl.
#
# Reloj: EngineFactory.now_ns() (JDEngine.now_ns() si hay extensión; si no Time.get_ticks_usec() · 1000).
class_name MockJDSource
extends RefCounted

const KIND_PHONE := "phone_server"
const KIND_REPLAY := "replay"
const KIND_RECORDER := "recorder"

const SAMPLE_PERIOD_NS := 5_000_000        # 200 Hz
const MAX_BACKLOG_NS := 2_000_000_000      # no se generan más de 2 s atrasados
const GRAVITY := 9.81
const FAKE_DEVICE_ID := 1
const FAKE_DEVICE_NAME := "Pixel (simulado)"
const LIMB_NAMES := ["left_hand", "right_hand", "left_foot", "right_foot"]
const LIMB_NONE := 255

## Semilla de todas las fuentes simuladas nuevas (cámbiala antes de crearlas).
static var default_seed: int = 20261106

## Segundos desde start() hasta que el móvil falso aparece conectado.
var connect_delay_s := 1.0
## Amplitud (m/s²) del balanceo suave del móvil falso.
var motion_amplitude := 2.0
## Desviación (m/s²) del ruido del acelerómetro.
var noise_amplitude := 0.15
## Pico (m/s²) de una sacudida simulada en el eje X.
var shake_amplitude := 30.0

var _kind := ""
var _rng := RandomNumberGenerator.new()
var _running := false
var _start_ns := 0
var _last_error := ""
var _limbs := {}                 # id (int) -> limb (int)

# servidor de móviles
var _port := 47800
var _code := 0
var _gen_ns := 0
var _ever_connected := false
var _shakes := PackedInt64Array()   # instantes (ns) de sacudidas simuladas, ordenados

# reproducción
var _speed := 1.0
var _rep_t := PackedFloat64Array()
var _rep_samples: Array = []        # {id, limb, a, g} sin t_ns
var _rep_devices: Array = []        # {id, name, kind}
var _rep_idx := 0

# grabador
var _inner: Object = null
var _out_path := ""
var _file: FileAccess = null
var _mapping := 0.0


func _init() -> void:
	_rng.seed = default_seed


## Reloj monotónico (ns): el mismo que usan SongClock y MockJDEngine (EngineFactory.now_ns()).
static func now_ns() -> int:
	return EngineFactory.now_ns()


# ------------------------------------------------------------------ creación (estáticos, como JDSource)

static func create_phone_server(port: int = 47800, code: int = 0) -> MockJDSource:
	var s := MockJDSource.new()
	s._kind = KIND_PHONE
	s._port = port if port > 0 else 47800
	s._code = code if code > 0 else s._rng.randi_range(1000, 9999)
	s._code = s._code % 10000
	s._limbs[FAKE_DEVICE_ID] = 1   # primer móvil = right_hand (phone_protocol.md)
	return s


static func create_replay(path: String, speed: float = 1.0) -> MockJDSource:
	if not FileAccess.file_exists(path):
		return null
	var s := MockJDSource.new()
	s._kind = KIND_REPLAY
	s._speed = speed if speed > 0.0 else 1.0
	s._load_session(path)
	return s


static func create_recorder(inner: Object, out_path: String) -> MockJDSource:
	if inner == null:
		return null
	var s := MockJDSource.new()
	s._kind = KIND_RECORDER
	s._inner = inner
	s._out_path = out_path
	return s


# ------------------------------------------------------------------ interfaz JDSource

func endpoint() -> String:
	if _kind != KIND_PHONE:
		return ""
	return "%s:%d" % [_lan_ip(), _port]


func pairing_code() -> int:
	return _code if _kind == KIND_PHONE else 0


func start() -> bool:
	# El error de carga de un replay se conserva para que start() lo detecte.
	if _kind != KIND_REPLAY:
		_last_error = ""
	match _kind:
		KIND_PHONE:
			if _port <= 0 or _port > 65535:
				_last_error = "Puerto no válido: %d" % _port
				return false
		KIND_REPLAY:
			if not _last_error.is_empty():
				return false
			_rep_idx = 0
		KIND_RECORDER:
			if _inner == null:
				_last_error = "Grabador sin fuente interior"
				return false
			if not bool(_inner.call(&"start")):
				_last_error = "La fuente interior no arrancó: %s" % str(_inner.call(&"last_error"))
				return false
			_open_recording()
	_running = true
	_start_ns = now_ns()
	_gen_ns = _start_ns
	return true


func stop() -> void:
	_running = false
	if _kind == KIND_RECORDER:
		if _inner != null:
			_inner.call(&"stop")
		if _file != null:
			_file.close()
			_file = null


func devices() -> Array[Dictionary]:
	var out: Array[Dictionary] = []
	match _kind:
		KIND_PHONE:
			if _running and now_ns() >= _connect_ns():
				_ever_connected = true
			if _ever_connected:
				var elapsed_s := float(now_ns() - _start_ns) * 1e-9
				out.append({
					"id": FAKE_DEVICE_ID,
					"name": FAKE_DEVICE_NAME,
					"kind": "phone",
					"limb": int(_limbs.get(FAKE_DEVICE_ID, 1)),
					"connected": _running,
					"battery": clampf(0.85 - elapsed_s / 36000.0, 0.0, 1.0),
					"sync_rtt_ms": 6.0 + _rng.randf() * 4.0,
				})
		KIND_REPLAY:
			for d: Dictionary in _rep_devices:
				var id: int = int(d["id"])
				out.append({
					"id": id,
					"name": str(d["name"]),
					"kind": str(d["kind"]),
					"limb": int(_limbs.get(id, LIMB_NONE)),
					"connected": _running,
					"battery": -1.0,
					"sync_rtt_ms": 0.0,
				})
		KIND_RECORDER:
			if _inner != null:
				var inner_devs: Array = _inner.call(&"devices")
				for d: Dictionary in inner_devs:
					out.append(d)
	return out


func assign_limb(id: int, limb: int) -> void:
	if _kind == KIND_RECORDER and _inner != null:
		_inner.call(&"assign_limb", id, limb)
		return
	_limbs[id] = limb


func set_recorder_mapping(mono_to_song_s: float) -> void:
	_mapping = mono_to_song_s


func poll_samples(max_count: int = 512) -> Array[Dictionary]:
	if max_count <= 0:
		max_count = 512
	match _kind:
		KIND_PHONE:
			return _poll_phone(max_count)
		KIND_REPLAY:
			return _poll_replay(max_count)
		KIND_RECORDER:
			return _poll_recorder(max_count)
	var empty: Array[Dictionary] = []
	return empty


func last_error() -> String:
	return _last_error


# ------------------------------------------------------------------ extras de la simulación

func set_seed(seed_value: int) -> void:
	_rng.seed = seed_value


## Programa `count` sacudidas: la i-ésima en first_click_ns + i·period_s + latency_s (+ ruido gaussiano
## de desviación jitter_s). Sirve para probar la calibración de entrada sin móvil.
func simulate_rhythm(first_click_ns: int, period_s: float, count: int, latency_s: float = 0.03, jitter_s: float = 0.012) -> void:
	for i in range(count):
		var t_s := float(i) * period_s + latency_s + _rng.randfn(0.0, jitter_s)
		_shakes.append(first_click_ns + int(t_s * 1e9))
	_shakes.sort()


## Añade una sacudida en t_ns (o ahora si t_ns < 0).
func inject_shake(t_ns: int = -1) -> void:
	_shakes.append(t_ns if t_ns >= 0 else now_ns())
	_shakes.sort()


# ------------------------------------------------------------------ internos: servidor de móviles

func _connect_ns() -> int:
	return _start_ns + int(connect_delay_s * 1e9)


func _poll_phone(max_count: int) -> Array[Dictionary]:
	var out: Array[Dictionary] = []
	if not _running:
		return out
	var now := now_ns()
	var connect_ns := _connect_ns()
	if now < connect_ns:
		_gen_ns = connect_ns
		return out
	if _gen_ns < connect_ns:
		_gen_ns = connect_ns
	if now - _gen_ns > MAX_BACKLOG_NS:
		_gen_ns = now - MAX_BACKLOG_NS
	while _gen_ns + SAMPLE_PERIOD_NS <= now and out.size() < max_count:
		_gen_ns += SAMPLE_PERIOD_NS
		out.append(_synth_sample(_gen_ns))
	_prune_shakes(_gen_ns - 1_000_000_000)
	return out


func _synth_sample(t_ns: int) -> Dictionary:
	var t := float(t_ns - _start_ns) * 1e-9
	var a := Vector3(
		motion_amplitude * sin(TAU * 0.5 * t),
		GRAVITY + motion_amplitude * 0.5 * sin(TAU * 1.0 * t),
		motion_amplitude * 0.3 * cos(TAU * 0.25 * t))
	a += Vector3(_rng.randfn(0.0, 1.0), _rng.randfn(0.0, 1.0), _rng.randfn(0.0, 1.0)) * noise_amplitude
	a.x += _shake_at(t_ns)
	var g := Vector3(_rng.randfn(0.0, 1.0), _rng.randfn(0.0, 1.0), _rng.randfn(0.0, 1.0)) * 0.05
	g.z += motion_amplitude * 0.3 * cos(TAU * 0.5 * t)
	return {
		"t_ns": t_ns,
		"id": FAKE_DEVICE_ID,
		"limb": int(_limbs.get(FAKE_DEVICE_ID, 1)),
		"a": a,
		"g": g,
	}


func _shake_at(t_ns: int) -> float:
	var acc := 0.0
	for s: int in _shakes:
		var d := float(t_ns - s) * 1e-9
		if d < -0.05:
			break
		if d <= 0.05:
			var x := d / 0.015
			acc += shake_amplitude * exp(-x * x)
	return acc


func _prune_shakes(before_ns: int) -> void:
	var keep := PackedInt64Array()
	for s: int in _shakes:
		if s >= before_ns:
			keep.append(s)
	_shakes = keep


static func _lan_ip() -> String:
	for ip: String in IP.get_local_addresses():
		if ip.begins_with("192.168.") or ip.begins_with("10."):
			return ip
		if ip.begins_with("172."):
			var parts := ip.split(".")
			if parts.size() == 4:
				var second: int = String(parts[1]).to_int()
				if second >= 16 and second <= 31:
					return ip
	return "127.0.0.1"


# ------------------------------------------------------------------ internos: reproducción

func _load_session(path: String) -> void:
	_rep_t = PackedFloat64Array()
	_rep_samples = []
	_rep_devices = []
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		_last_error = "No se puede abrir %s" % path
		return
	var header_seen := false
	var line_no := 0
	while not f.eof_reached():
		var line := f.get_line().strip_edges()
		line_no += 1
		if line.is_empty() or line.begins_with("#"):
			continue
		var v: Variant = JSON.parse_string(line)
		if typeof(v) != TYPE_DICTIONARY:
			_last_error = "%s:%d: JSON no válido" % [path, line_no]
			return
		var d: Dictionary = v
		if not header_seen:
			header_seen = true
			if str(d.get("format", "")) != "jd-session":
				_last_error = "%s: la cabecera debe tener format \"jd-session\"" % path
				return
			var devs: Variant = d.get("devices", [])
			if typeof(devs) == TYPE_ARRAY:
				for dv: Variant in devs:
					if typeof(dv) != TYPE_DICTIONARY:
						continue
					var dd: Dictionary = dv
					var id := int(dd.get("source_id", 0))
					_rep_devices.append({"id": id, "name": str(dd.get("name", "?")), "kind": str(dd.get("kind", "phone"))})
					_limbs[id] = _limb_from_name(str(dd.get("limb", "")))
			continue
		if not d.has("t"):
			continue
		var smp := {
			"id": int(d.get("src", 0)),
			"limb": _limb_from_name(str(d.get("limb", ""))),
			"a": _vec3(d.get("a", null)),
			"g": _vec3(d.get("g", null)),
		}
		_rep_t.append(float(d["t"]))
		_rep_samples.append(smp)
	f.close()
	if not header_seen:
		_last_error = "%s: archivo vacío" % path


func _poll_replay(max_count: int) -> Array[Dictionary]:
	var out: Array[Dictionary] = []
	if not _running or _rep_t.is_empty():
		return out
	var t_first: float = _rep_t[0]
	var elapsed := float(now_ns() - _start_ns) * 1e-9 * _speed
	while _rep_idx < _rep_t.size() and out.size() < max_count:
		var rel: float = _rep_t[_rep_idx] - t_first
		if rel > elapsed:
			break
		var smp: Dictionary = (_rep_samples[_rep_idx] as Dictionary).duplicate()
		smp["t_ns"] = _start_ns + int(rel / _speed * 1e9)
		var id: int = int(smp["id"])
		smp["limb"] = int(_limbs.get(id, smp["limb"]))
		out.append(smp)
		_rep_idx += 1
	return out


static func _limb_from_name(n: String) -> int:
	var i := LIMB_NAMES.find(n)
	return i if i >= 0 else LIMB_NONE


static func _vec3(v: Variant) -> Vector3:
	if typeof(v) == TYPE_ARRAY and (v as Array).size() >= 3:
		var arr: Array = v
		return Vector3(float(arr[0]), float(arr[1]), float(arr[2]))
	return Vector3.ZERO


# ------------------------------------------------------------------ internos: grabador

func _open_recording() -> void:
	_file = FileAccess.open(_out_path, FileAccess.WRITE)
	if _file == null:
		_last_error = "No se puede escribir %s" % _out_path
		return
	var devs: Array = []
	var inner_devs: Array = _inner.call(&"devices")
	for d: Dictionary in inner_devs:
		var limb := int(d.get("limb", LIMB_NONE))
		devs.append({
			"source_id": int(d.get("id", 0)),
			"name": str(d.get("name", "?")),
			"kind": str(d.get("kind", "phone")),
			"limb": LIMB_NAMES[limb] if limb >= 0 and limb < LIMB_NAMES.size() else null,
		})
	var header := {
		"format": "jd-session",
		"version": 1,
		"time_base": "song",
		"song_id": null,
		"recorded_at": Time.get_datetime_string_from_system(true) + "Z",
		"note": "grabado con MockJDSource",
		"devices": devs,
	}
	_file.store_line(JSON.stringify(header))


func _poll_recorder(max_count: int) -> Array[Dictionary]:
	var out: Array[Dictionary] = []
	if _inner == null:
		return out
	var batch: Array = _inner.call(&"poll_samples", max_count)
	for smp: Dictionary in batch:
		out.append(smp)
		if _file != null:
			var limb := int(smp.get("limb", LIMB_NONE))
			var a: Vector3 = smp.get("a", Vector3.ZERO)
			var g: Vector3 = smp.get("g", Vector3.ZERO)
			var line := {
				"t": float(int(smp["t_ns"])) * 1e-9 + _mapping,
				"src": int(smp.get("id", 0)),
				"limb": LIMB_NAMES[limb] if limb >= 0 and limb < LIMB_NAMES.size() else null,
				"a": [a.x, a.y, a.z],
				"g": [g.x, g.y, g.z],
			}
			_file.store_line(JSON.stringify(line, "", false, true))
	return out
