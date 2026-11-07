# requires: Profile
# Tests de caja negra de Profile (contracts/game_logic_api.md, docs/06 fila B9).
# Nunca toca user://profile.cfg (el perfil real del jugador): todo va a user://t2_tests/profile/.
extends "res://tests/test_base.gd"

var _dir := ""


func before_each() -> void:
	_dir = tmp_dir("profile")


func after_each() -> void:
	remove_tree(_dir)


func _assert_defaults(p: Profile, msg: String) -> void:
	assert_eq(p.audio_offset, 0.0, msg + ": audio_offset")
	assert_eq(p.visual_offset, 0.0, msg + ": visual_offset")
	assert_eq(p.input_offset, 0.0, msg + ": input_offset")
	assert_eq(p.perfect_ms, 50.0, msg + ": perfect_ms")
	assert_eq(p.good_ms, 100.0, msg + ": good_ms")
	assert_eq(p.foot_scale, 1.5, msg + ": foot_scale")
	assert_eq(p.limb_by_device.size(), 0, msg + ": limb_by_device vacío")
	assert_eq(p.phone_port, 47800, msg + ": phone_port")


## Perfil con valores no redondos en todos los campos.
func _filled() -> Profile:
	var p := Profile.new()
	p.audio_offset = 0.0123456789012345
	p.visual_offset = -0.0337
	p.input_offset = 0.1 + 0.2          # 0,30000000000000004: prueba la precisión completa
	p.perfect_ms = 42.5
	p.good_ms = 87.25
	p.foot_scale = 1.75
	p.limb_by_device = {"Pixel 7": 2, "Galaxy S23": 0, "Móvil de Ana": 3}
	p.phone_port = 50123
	return p


func test_defaults() -> void:
	_assert_defaults(Profile.new(), "Profile.new()")
	done()


func test_round_trip_exact() -> void:
	var path := _dir.path_join("rt.cfg")
	var p := _filled()
	assert_eq(p.save(path), OK, "save devuelve OK")
	assert_true(FileAccess.file_exists(path), "el archivo existe")
	var q := Profile.new()
	assert_true(q.load(path), "load devuelve true")
	assert_eq(q.audio_offset, p.audio_offset, "audio_offset exacto")
	assert_eq(q.visual_offset, p.visual_offset, "visual_offset exacto")
	assert_eq(q.input_offset, p.input_offset, "input_offset exacto (0,1 + 0,2)")
	assert_eq(q.perfect_ms, p.perfect_ms, "perfect_ms")
	assert_eq(q.good_ms, p.good_ms, "good_ms")
	assert_eq(q.foot_scale, p.foot_scale, "foot_scale")
	assert_eq(q.phone_port, 50123, "phone_port")
	assert_type(q.phone_port, TYPE_INT, "phone_port es int")
	assert_eq(q.limb_by_device.size(), 3, "limb_by_device: 3 dispositivos")
	for dev: String in ["Pixel 7", "Galaxy S23", "Móvil de Ana"]:
		if assert_true(q.limb_by_device.has(dev), "limb_by_device tiene \"%s\"" % dev):
			assert_eq(int(q.limb_by_device[dev]), int(p.limb_by_device[dev]), "limb de " + dev)
	done()


func test_round_trip_twice_stable() -> void:
	var a := _dir.path_join("a.cfg")
	var b := _dir.path_join("b.cfg")
	assert_eq(_filled().save(a), OK)
	var q := Profile.new()
	q.load(a)
	assert_eq(q.save(b), OK)
	var r := Profile.new()
	assert_true(r.load(b), "segunda carga")
	assert_eq(r.input_offset, 0.1 + 0.2, "sin pérdida tras dos idas y vueltas")
	assert_eq(r.phone_port, 50123)
	done()


func test_default_values_round_trip() -> void:
	var path := _dir.path_join("def.cfg")
	assert_eq(Profile.new().save(path), OK)
	var q := Profile.new()
	assert_true(q.load(path))
	_assert_defaults(q, "defaults guardados y cargados")
	done()


func test_missing_file() -> void:
	var p := _filled()
	assert_false(p.load(_dir.path_join("no_existe.cfg")), "archivo ausente → false")
	_assert_defaults(p, "ausente")
	done()


func _check_corrupt(content: String, what: String) -> void:
	var path := _dir.path_join("bad.cfg")
	assert_true(write_text(path, content), "escribir el archivo corrupto")
	var p := _filled()
	assert_false(p.load(path), what + " → false")
	_assert_defaults(p, what)


func test_corrupt_not_json() -> void:
	_check_corrupt("{{{ esto no es JSON", "JSON roto")
	done()


func test_corrupt_empty() -> void:
	_check_corrupt("", "archivo vacío")
	done()


func test_corrupt_wrong_root_type() -> void:
	_check_corrupt("[1, 2, 3]", "raíz lista")
	done()


func test_corrupt_wrong_field_type() -> void:
	_check_corrupt("{\"audio_offset\": \"mucho\", \"phone_port\": 47801}", "campo con tipo erróneo")
	done()


func test_corrupt_binary_garbage() -> void:
	var path := _dir.path_join("bin.cfg")
	var f := FileAccess.open(path, FileAccess.WRITE)
	if assert_not_null(f, "abrir para escribir"):
		f.store_buffer(PackedByteArray([0, 255, 1, 254, 128, 0, 10, 13, 200]))
		f.close()
	var p := _filled()
	assert_false(p.load(path), "basura binaria → false")
	_assert_defaults(p, "basura binaria")
	done()


func test_phone_port_default_and_custom() -> void:
	var p := Profile.new()
	assert_eq(p.phone_port, 47800, "por defecto 47800 (phone_protocol.md)")
	p.phone_port = 47900
	var path := _dir.path_join("port.cfg")
	assert_eq(p.save(path), OK)
	var q := Profile.new()
	assert_true(q.load(path))
	assert_eq(q.phone_port, 47900)
	done()


func test_load_after_modification_resets_missing_keys() -> void:
	# Un archivo válido con solo algunas claves: el resto toma el valor por defecto.
	# (Contrato: valores por defecto si falta; el formato interno es de D, así que se genera con save.)
	var path := _dir.path_join("part.cfg")
	var p := Profile.new()
	p.input_offset = 0.033
	assert_eq(p.save(path), OK)
	var q := _filled()
	assert_true(q.load(path), "load de un perfil válido")
	assert_eq(q.input_offset, 0.033)
	assert_eq(q.audio_offset, 0.0, "no arrastra valores del objeto anterior")
	assert_eq(q.limb_by_device.size(), 0, "no arrastra limb_by_device del objeto anterior")
	done()
