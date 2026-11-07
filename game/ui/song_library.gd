# song_library.gd — lista los paquetes de canción (contracts/song_package.md, agente E).
# Busca en user://songs/ y en carpetas extra (por defecto <repo>/data/songs).
# Todas las rutas devueltas son ABSOLUTAS del sistema (las necesita el motor en load_song).
class_name SongLibrary
extends RefCounted

const LIMB_NAMES := ["left_hand", "right_hand", "left_foot", "right_foot"]
const USER_SONGS_DIR := "user://songs"

## Carpetas extra a explorar (rutas absolutas o res:// / user://).
var extra_dirs: PackedStringArray = PackedStringArray()


func _init() -> void:
	extra_dirs.append(default_extra_dir())


## <repo>/data/songs, resuelto desde la carpeta del proyecto (game/).
## En un juego exportado esta carpeta normalmente no existe y simplemente se ignora.
static func default_extra_dir() -> String:
	var base := ProjectSettings.globalize_path("res://")
	if base.is_empty():
		return ""
	return base.path_join("../data/songs").simplify_path()


static func limb_from_name(n: String) -> int:
	return LIMB_NAMES.find(n)


static func limb_name(limb: int) -> String:
	if limb >= 0 and limb < LIMB_NAMES.size():
		return LIMB_NAMES[limb]
	return "?"


## Explora todas las carpetas. Devuelve paquetes (ver load_package), válidos e inválidos,
## ordenados por título. Si un id aparece en varias carpetas gana user://songs.
func scan() -> Array[Dictionary]:
	var result: Array[Dictionary] = []
	var seen := {}
	var roots: Array[Dictionary] = [{"dir": ProjectSettings.globalize_path(USER_SONGS_DIR), "origin": "user"}]
	for d: String in extra_dirs:
		if d.is_empty():
			continue
		var abs_d: String = d
		if d.begins_with("res://") or d.begins_with("user://"):
			abs_d = ProjectSettings.globalize_path(d)
		roots.append({"dir": abs_d, "origin": "extra"})
	for root in roots:
		var root_dir: String = String(root["dir"]).simplify_path()
		if not DirAccess.dir_exists_absolute(root_dir):
			continue
		var da := DirAccess.open(root_dir)
		if da == null:
			continue
		var subdirs := da.get_directories()
		subdirs.sort()
		for sub: String in subdirs:
			if sub.begins_with(".") or seen.has(sub):
				continue
			var pkg_dir := root_dir.path_join(sub)
			if not FileAccess.file_exists(pkg_dir.path_join("choreo.json")):
				continue
			var pkg := load_package(pkg_dir)
			pkg["origin"] = root["origin"]
			seen[sub] = true
			result.append(pkg)
	result.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return _title_less(a, b))
	return result


static func _title_less(a: Dictionary, b: Dictionary) -> bool:
	return String(a["title"]).naturalnocasecmp_to(String(b["title"])) < 0


## Solo los paquetes válidos.
static func only_valid(pkgs: Array) -> Array:
	var out: Array = []
	for p: Variant in pkgs:
		if p is Dictionary and bool(p.get("ok", false)):
			out.append(p)
	return out


## Carga y valida lo mínimo de un paquete. Devuelve:
## {id, dir, ok, errors: PackedStringArray, title, artist, bpm, offset_s, duration_s,
##  audio_path, video_path ("" si no hay), steps: Array[Dictionary], step_times: PackedFloat64Array}
## Cada paso: {t, dur, limbs: PackedInt32Array, limb_names: PackedStringArray, icon, kind}.
static func load_package(dir: String) -> Dictionary:
	var pkg := _new_pkg(dir)
	var errs: Array[String] = []
	_fill(dir, pkg, errs)
	pkg["errors"] = PackedStringArray(errs)
	pkg["ok"] = errs.is_empty()
	return pkg


static func _new_pkg(dir: String) -> Dictionary:
	return {
		"id": dir.get_file(), "dir": dir, "ok": false, "errors": PackedStringArray(),
		"title": dir.get_file(), "artist": "", "bpm": 0.0, "offset_s": 0.0, "duration_s": 0.0,
		"audio_path": "", "video_path": "", "steps": [], "step_times": PackedFloat64Array(),
	}


# Rellena pkg; los errores van a `errors` (Array: se pasa por referencia, PackedStringArray no).
static func _fill(dir: String, pkg: Dictionary, errors: Array[String]) -> void:
	var choreo_path := dir.path_join("choreo.json")
	var text := FileAccess.get_file_as_string(choreo_path)
	if text.is_empty():
		errors.append("choreo.json: no se puede leer (%s)" % error_string(FileAccess.get_open_error()))
		return
	var json := JSON.new()
	if json.parse(text) != OK:
		errors.append("choreo.json:%d: JSON no válido: %s" % [json.get_error_line(), json.get_error_message()])
		return
	var root = json.data
	if not (root is Dictionary):
		errors.append("choreo.json: la raíz debe ser un objeto")
		return
	var fmt = root.get("format")
	if not (fmt is String and fmt == "jd-choreo"):
		errors.append("format: debe ser \"jd-choreo\"")
	var ver = root.get("version")
	if not ((ver is float or ver is int) and int(ver) == 1):
		errors.append("version: debe ser 1")
	var song = root.get("song")
	if not (song is Dictionary):
		errors.append("song: falta o no es un objeto")
		return
	var title = song.get("title")
	if title is String and not title.strip_edges().is_empty():
		pkg["title"] = title
	else:
		errors.append("song.title: debe ser un texto no vacío")
	var artist = song.get("artist")
	if artist is String:
		pkg["artist"] = artist
	pkg["bpm"] = _num(song.get("bpm"), 0.0)
	pkg["offset_s"] = _num(song.get("offset_s"), 0.0)
	pkg["duration_s"] = _num(song.get("duration_s"), 0.0)
	if float(pkg["duration_s"]) <= 0.0:
		errors.append("song.duration_s: debe ser > 0")
	var audio = song.get("audio")
	if audio is String and _safe_rel(audio):
		var ap := dir.path_join(audio)
		if FileAccess.file_exists(ap):
			pkg["audio_path"] = ap
		else:
			errors.append("song.audio: no existe el archivo %s" % audio)
	else:
		errors.append("song.audio: debe ser una ruta relativa")
	var video = song.get("video")
	if video is String and _safe_rel(video):
		var vp := dir.path_join(video)
		if FileAccess.file_exists(vp) and vp.get_extension().to_lower() == "ogv":
			pkg["video_path"] = vp
		# Vídeo ausente no es error: es opcional.
	var steps_v = root.get("steps")
	if not (steps_v is Array):
		errors.append("steps: debe ser una lista")
		return
	var steps: Array = pkg["steps"]
	var times := PackedFloat64Array()
	var prev_t := -INF
	for i in range(steps_v.size()):
		var s = steps_v[i]
		if not (s is Dictionary):
			errors.append("steps[%d]: debe ser un objeto" % i)
			continue
		var t = s.get("t")
		if not (t is float or t is int):
			errors.append("steps[%d].t: debe ser un número" % i)
			continue
		var tf := float(t)
		if tf < prev_t:
			errors.append("steps[%d].t: debe ser >= steps[%d].t" % [i, i - 1])
		prev_t = maxf(prev_t, tf)
		var limbs := PackedInt32Array()
		var lnames := PackedStringArray()
		var lv = s.get("limbs")
		if lv is Array:
			for ln in lv:
				var li := limb_from_name(str(ln))
				if li < 0:
					errors.append("steps[%d].limbs: extremidad desconocida \"%s\"" % [i, str(ln)])
				elif not limbs.has(li):
					limbs.append(li)
					lnames.append(str(ln))
		if limbs.is_empty():
			errors.append("steps[%d].limbs: debe ser una lista no vacía" % i)
		if tf < 0.0:
			errors.append("steps[%d].t: debe ser >= 0" % i)
		var icon = s.get("icon")
		var kind = s.get("kind")
		if kind != null and not (kind is String and (kind == "accent" or kind == "move")):
			errors.append("steps[%d].kind: debe ser \"accent\" o \"move\"" % i)
		steps.append({
			"t": tf, "dur": maxf(_num(s.get("dur"), 0.0), 0.0), "limbs": limbs, "limb_names": lnames,
			"icon": icon if icon is String else "", "kind": kind if kind is String else "accent",
		})
		times.append(tf)
	pkg["step_times"] = times


static func _num(v: Variant, def: float) -> float:
	if v is float or v is int:
		return float(v)
	return def


static func _safe_rel(p: String) -> bool:
	return not p.is_empty() and not p.begins_with("/") and not p.contains("\\") \
		and not p.contains(":") and not p.contains("..")
