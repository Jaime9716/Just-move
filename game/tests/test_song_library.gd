# requires: SongLibrary
# SongLibrary (agente E, contracts/song_package.md) con paquetes temporales en user://t2_tests/songs.
extends "res://tests/test_base.gd"

const SONG_ID := "t2_tmp_song_9f3a"

var _root := ""


func before_each() -> void:
	_root = tmp_dir("songs")


func after_each() -> void:
	remove_tree(_root)


func _choreo(title: String, steps: Array) -> Dictionary:
	return {
		"format": "jd-choreo", "version": 1,
		"song": {"title": title, "artist": "Artista T2", "audio": "audio.ogg", "video": "video.ogv",
				"bpm": 100.0, "offset_s": 0.5, "duration_s": 42.0},
		"steps": steps,
		"references": {},
	}


func _good_steps() -> Array:
	return [
		{"t": 1.0, "dur": 0.25, "limbs": ["right_hand"], "icon": "punch_right", "kind": "accent"},
		{"t": 2.0, "dur": 1.0, "limbs": ["left_hand", "right_hand"], "icon": "wave", "kind": "move"},
		{"t": 3.5, "dur": 0.25, "limbs": ["right_foot"], "icon": "rarísimo", "kind": "accent"},
	]


## Crea <root>/<id>/ con choreo.json y (opcional) audio.ogg / video.ogv. Devuelve la ruta user://.
func _make_pkg(id: String, choreo: Variant, with_audio: bool = true, with_video: bool = false) -> String:
	var dir := _root.path_join(id)
	DirAccess.make_dir_recursive_absolute(dir)
	var text: String = choreo if typeof(choreo) == TYPE_STRING else JSON.stringify(choreo, "  ")
	write_text(dir.path_join("choreo.json"), text)
	if with_audio:
		write_text(dir.path_join("audio.ogg"), "falso")
	if with_video:
		write_text(dir.path_join("video.ogv"), "falso")
	return dir


func test_load_valid_package() -> void:
	var dir := ProjectSettings.globalize_path(_make_pkg(SONG_ID, _choreo("Canción T2", _good_steps()), true, true))
	var pkg := SongLibrary.load_package(dir)
	assert_true(bool(pkg.get("ok", false)), "paquete válido (errores: %s)" % str(pkg.get("errors", [])))
	assert_eq(str(pkg.get("id", "")), SONG_ID, "id = nombre de la carpeta")
	assert_eq(str(pkg.get("title", "")), "Canción T2")
	assert_eq(str(pkg.get("artist", "")), "Artista T2")
	assert_eq(float(pkg.get("bpm", 0.0)), 100.0)
	assert_eq(float(pkg.get("offset_s", 0.0)), 0.5)
	assert_eq(float(pkg.get("duration_s", 0.0)), 42.0)
	assert_true(str(pkg.get("audio_path", "")).ends_with("audio.ogg"), "audio_path")
	assert_true(str(pkg.get("video_path", "")).ends_with("video.ogv"), "video_path")
	assert_true(FileAccess.file_exists(str(pkg.get("audio_path", ""))), "audio_path existe")
	var steps: Array = pkg.get("steps", [])
	assert_eq(steps.size(), 3, "3 pasos")
	var times: PackedFloat64Array = pkg.get("step_times", PackedFloat64Array())
	assert_eq(times, PackedFloat64Array([1.0, 2.0, 3.5]), "step_times")
	if steps.size() == 3:
		var s1: Dictionary = steps[1]
		assert_eq(s1.get("limbs"), PackedInt32Array([0, 1]), "limbs left_hand,right_hand → 0,1")
		assert_eq(str(s1.get("kind", "")), "move")
		var s2: Dictionary = steps[2]
		assert_eq(s2.get("limbs"), PackedInt32Array([3]), "right_foot → 3")
		assert_eq(str(s2.get("icon", "")), "rarísimo", "icono desconocido se conserva (la UI pone el genérico)")
	done()


func test_video_optional() -> void:
	var dir := _make_pkg(SONG_ID, _choreo("Sin vídeo", _good_steps()), true, false)
	var pkg := SongLibrary.load_package(dir)
	assert_true(bool(pkg.get("ok", false)), "sin video.ogv sigue siendo válido")
	assert_eq(str(pkg.get("video_path", "x")), "", "video_path vacío")
	done()


func test_missing_audio_invalid() -> void:
	var dir := _make_pkg(SONG_ID, _choreo("Sin audio", _good_steps()), false)
	var pkg := SongLibrary.load_package(dir)
	assert_false(bool(pkg.get("ok", true)), "sin audio.ogg → no válido")
	var errs: PackedStringArray = pkg.get("errors", PackedStringArray())
	assert_gt(float(errs.size()), 0.0, "con mensaje de error")
	done()


func _errors_text(pkg: Dictionary) -> String:
	var errs: PackedStringArray = pkg.get("errors", PackedStringArray())
	return " | ".join(errs)


func test_invalid_packages_have_clear_errors() -> void:
	var unordered := _good_steps()
	unordered[2]["t"] = 0.5
	var p1 := SongLibrary.load_package(_make_pkg("unordered", _choreo("Desordenada", unordered)))
	assert_false(bool(p1["ok"]), "pasos desordenados")
	assert_true(_errors_text(p1).contains("steps[2].t"), "ruta JSON del error (%s)" % _errors_text(p1))

	var bad_limb := _good_steps()
	bad_limb[0]["limbs"] = ["cabeza"]
	var p2 := SongLibrary.load_package(_make_pkg("bad_limb", _choreo("Extremidad mala", bad_limb)))
	assert_false(bool(p2["ok"]), "extremidad desconocida")
	assert_true(_errors_text(p2).contains("steps[0].limbs"), "ruta JSON (%s)" % _errors_text(p2))

	var no_limbs := _good_steps()
	no_limbs[1]["limbs"] = []
	var p3 := SongLibrary.load_package(_make_pkg("no_limbs", _choreo("Sin extremidades", no_limbs)))
	assert_false(bool(p3["ok"]), "limbs vacío")

	var p4 := SongLibrary.load_package(_make_pkg("broken", "{ \"format\": \"jd-choreo\", "))
	assert_false(bool(p4["ok"]), "JSON roto")
	assert_true(_errors_text(p4).contains("choreo.json"), "menciona choreo.json (%s)" % _errors_text(p4))

	var wrong := _choreo("Formato malo", _good_steps())
	wrong["format"] = "otra-cosa"
	var p5 := SongLibrary.load_package(_make_pkg("wrong_format", wrong))
	assert_false(bool(p5["ok"]), "format distinto de jd-choreo")

	var bad_path := _choreo("Ruta mala", _good_steps())
	bad_path["song"]["audio"] = "../../secreto.ogg"
	var p6 := SongLibrary.load_package(_make_pkg("bad_path", bad_path))
	assert_false(bool(p6["ok"]), "audio fuera del paquete (..) no se acepta")
	done()


func test_scan_finds_temp_package() -> void:
	_make_pkg(SONG_ID, _choreo("Canción escaneada T2", _good_steps()))
	_make_pkg("t2_tmp_invalid_9f3a", _choreo("", _good_steps()))     # título vacío → inválido
	DirAccess.make_dir_recursive_absolute(_root.path_join("t2_tmp_sin_choreo"))   # se ignora
	var lib := SongLibrary.new()
	lib.extra_dirs = PackedStringArray([_root])
	var all := lib.scan()
	var mine := {}
	for p: Dictionary in all:
		var id := str(p.get("id", ""))
		if id.begins_with("t2_tmp_"):
			mine[id] = p
	assert_true(mine.has(SONG_ID), "scan encuentra el paquete temporal")
	assert_true(mine.has("t2_tmp_invalid_9f3a"), "scan devuelve también los inválidos")
	assert_false(mine.has("t2_tmp_sin_choreo"), "carpeta sin choreo.json se ignora")
	if mine.has(SONG_ID):
		var p: Dictionary = mine[SONG_ID]
		assert_true(bool(p["ok"]))
		assert_true(str(p["dir"]).is_absolute_path(), "dir es ruta absoluta del sistema (%s)" % str(p["dir"]))
	if mine.has("t2_tmp_invalid_9f3a"):
		assert_false(bool(mine["t2_tmp_invalid_9f3a"]["ok"]))
	var valid := SongLibrary.only_valid(all)
	for v: Variant in valid:
		var vd: Dictionary = v
		assert_true(bool(vd.get("ok", false)), "only_valid solo deja válidos")
	done()


func test_limb_names() -> void:
	assert_eq(SongLibrary.limb_from_name("left_hand"), 0)
	assert_eq(SongLibrary.limb_from_name("right_hand"), 1)
	assert_eq(SongLibrary.limb_from_name("left_foot"), 2)
	assert_eq(SongLibrary.limb_from_name("right_foot"), 3)
	assert_eq(SongLibrary.limb_from_name("nariz"), -1)
	assert_eq(SongLibrary.limb_name(3), "right_foot")
	done()
