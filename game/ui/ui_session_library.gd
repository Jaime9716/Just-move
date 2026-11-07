# ui_session_library.gd — lista las sesiones grabadas (.jsonl, contracts/session_format.md) que se
# pueden reproducir como entrada cuando no hay móviles (agente E).
# Busca en user://sessions/ y en <repo>/data/sessions/ (subcarpetas incluidas).
class_name UiSessionLibrary
extends RefCounted


static func default_dirs() -> PackedStringArray:
	var out := PackedStringArray()
	out.append(ProjectSettings.globalize_path("user://sessions"))
	var base := ProjectSettings.globalize_path("res://")
	if not base.is_empty():
		out.append(base.path_join("../data/sessions").simplify_path())
	return out


## Devuelve diccionarios {path, name, song_id, note, time_base, first_t} ordenados por nombre.
static func scan(dirs: PackedStringArray = PackedStringArray(), max_depth: int = 3) -> Array:
	if dirs.is_empty():
		dirs = default_dirs()
	var out: Array = []
	for d: String in dirs:
		_scan_dir(d, d, max_depth, out)
	out.sort_custom(_name_less)
	return out


static func _name_less(a: Dictionary, b: Dictionary) -> bool:
	return String(a["name"]).naturalnocasecmp_to(String(b["name"])) < 0


static func _scan_dir(root: String, dir: String, depth: int, out: Array) -> void:
	if depth < 0 or not DirAccess.dir_exists_absolute(dir):
		return
	var da := DirAccess.open(dir)
	if da == null:
		return
	for f: String in da.get_files():
		if not f.ends_with(".jsonl"):
			continue
		var info := read_header(dir.path_join(f))
		if bool(info["ok"]):
			var rel := dir.path_join(f).trim_prefix(root).trim_prefix("/")
			info["name"] = rel
			out.append(info)
	for sub: String in da.get_directories():
		if not sub.begins_with("."):
			_scan_dir(root, dir.path_join(sub), depth - 1, out)


## Lee la cabecera y la primera muestra. ok = false si no es una sesión jd-session.
static func read_header(path: String) -> Dictionary:
	var info := {
		"ok": false, "path": path, "name": path.get_file(), "song_id": "", "note": "",
		"time_base": "song", "first_t": 0.0,
	}
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		return info
	var header: Variant = null
	var lines_read := 0
	while not f.eof_reached() and lines_read < 64:
		var line := f.get_line().strip_edges()
		lines_read += 1
		if line.is_empty() or line.begins_with("#"):
			continue
		var parsed: Variant = JSON.parse_string(line)
		if header == null:
			if not (parsed is Dictionary) or parsed.get("format", "") != "jd-session":
				return info
			header = parsed
			info["ok"] = true
			var sid: Variant = parsed.get("song_id")
			info["song_id"] = sid if sid is String else ""
			var note: Variant = parsed.get("note")
			info["note"] = note if note is String else ""
			var tb: Variant = parsed.get("time_base")
			info["time_base"] = tb if tb is String else "song"
		else:
			if parsed is Dictionary and (parsed.get("t") is float or parsed.get("t") is int):
				info["first_t"] = float(parsed.get("t"))
			break
	return info
