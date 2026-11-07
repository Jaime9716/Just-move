# profile.gd — perfil del jugador / equipo (B9). Ver contracts/game_logic_api.md.
# Se guarda como JSON (con precisión completa para que la ida y vuelta sea exacta) aunque el
# archivo se llame profile.cfg.
class_name Profile
extends RefCounted

const DEFAULT_PATH := "user://profile.cfg"
const FORMAT := "jd-profile"
const VERSION := 1

var audio_offset := 0.0    ## s
var visual_offset := 0.0   ## s
var input_offset := 0.0    ## s
var perfect_ms := 50.0
var good_ms := 100.0
var foot_scale := 1.5
var limb_by_device: Dictionary = {}   ## nombre de dispositivo (String) → limb (int, JD_Limb)
var phone_port := 47800


func reset_defaults() -> void:
	audio_offset = 0.0
	visual_offset = 0.0
	input_offset = 0.0
	perfect_ms = 50.0
	good_ms = 100.0
	foot_scale = 1.5
	limb_by_device = {}
	phone_port = 47800


func to_dict() -> Dictionary:
	return {
		"format": FORMAT,
		"version": VERSION,
		"audio_offset": audio_offset,
		"visual_offset": visual_offset,
		"input_offset": input_offset,
		"perfect_ms": perfect_ms,
		"good_ms": good_ms,
		"foot_scale": foot_scale,
		"limb_by_device": limb_by_device.duplicate(),
		"phone_port": phone_port,
	}


## Crea un perfil y lo carga de `path` (valores por defecto si falta o está corrupto).
static func load_or_default(path: String = DEFAULT_PATH) -> Profile:
	var p := Profile.new()
	p.load(path)
	return p


func save(path: String = DEFAULT_PATH) -> Error:
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		return FileAccess.get_open_error()
	# sort_keys = true, full_precision = true → floats exactos al recargar.
	f.store_string(JSON.stringify(to_dict(), "\t", true, true))
	f.store_string("\n")
	var err := f.get_error()
	f.close()
	return err


## Carga el perfil. Si falta o está corrupto, deja los valores por defecto y devuelve false.
## Las claves ausentes (archivo de una versión anterior) toman su valor por defecto.
func load(path: String = DEFAULT_PATH) -> bool:
	reset_defaults()
	if not FileAccess.file_exists(path):
		return false
	var text := FileAccess.get_file_as_string(path)
	if text.is_empty():
		return false
	var json := JSON.new()
	if json.parse(text) != OK:
		return false
	var data: Variant = json.data
	if typeof(data) != TYPE_DICTIONARY:
		return false
	var d: Dictionary = data
	if d.get("format", FORMAT) != FORMAT:
		return false

	var values := {}
	for key: String in ["audio_offset", "visual_offset", "input_offset", "perfect_ms", "good_ms", "foot_scale", "phone_port"]:
		if d.has(key):
			if not _is_number(d[key]):
				reset_defaults()
				return false
			values[key] = float(d[key])
	var limbs := {}
	if d.has("limb_by_device"):
		if typeof(d["limb_by_device"]) != TYPE_DICTIONARY:
			return false
		var raw: Dictionary = d["limb_by_device"]
		for name: Variant in raw:
			if not _is_number(raw[name]):
				return false
			limbs[str(name)] = int(raw[name])

	audio_offset = values.get("audio_offset", audio_offset)
	visual_offset = values.get("visual_offset", visual_offset)
	input_offset = values.get("input_offset", input_offset)
	perfect_ms = values.get("perfect_ms", perfect_ms)
	good_ms = values.get("good_ms", good_ms)
	foot_scale = values.get("foot_scale", foot_scale)
	phone_port = int(values.get("phone_port", float(phone_port)))
	if phone_port <= 0 or phone_port > 65535:
		reset_defaults()
		return false
	limb_by_device = limbs
	return true


static func _is_number(v: Variant) -> bool:
	return typeof(v) == TYPE_FLOAT or typeof(v) == TYPE_INT
