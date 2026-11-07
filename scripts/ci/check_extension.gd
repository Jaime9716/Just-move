# Prueba de humo de la GDExtension (agente G). La CI la copia a game/_ci_check_extension.gd y ejecuta:
#   godot --headless --path game -s res://_ci_check_extension.gd -- <ruta absoluta de data/>
# Solo usa ClassDB/call() para que el script compile aunque la extensión no esté cargada.
extends SceneTree

var _fails := 0

func _check(cond: bool, what: String) -> void:
	if cond:
		print("  ok   ", what)
	else:
		_fails += 1
		printerr("  FALLO ", what)

## Método estático de una clase nativa: ClassDB.class_call_static (Godot 4.4+) o, si no existe,
## a través de una instancia (Godot permite llamar a un método estático sobre una instancia).
func _static(cls: String, method: String, args: Array) -> Variant:
	if ClassDB.has_method("class_call_static"):
		return ClassDB.callv("class_call_static", [cls, method] + args)
	var inst: Object = ClassDB.instantiate(cls)
	return inst.callv(method, args)

func _init() -> void:
	print("== Comprobando la GDExtension jd_godot ==")
	var loaded := ClassDB.class_exists("JDEngine") and ClassDB.class_exists("JDSource")
	_check(loaded, "clases JDEngine y JDSource registradas")
	if not loaded:
		printerr("La extensión no se cargó: ¿está game/bin/libjd_godot.so y se generó .godot/extension_list.cfg?")
		quit(1)
		return
	var data_dir := ""
	var args := OS.get_cmdline_user_args()
	if args.size() > 0:
		data_dir = args[0]

	var eng: Object = ClassDB.instantiate("JDEngine")
	var t0: int = _static("JDEngine", "now_ns", [])
	var t1: int = eng.call("now_ns")
	_check(t0 > 0 and t1 >= t0, "JDEngine.now_ns() monotónico (%d)" % t0)
	var err: String = eng.call("load_song", "/no/existe")
	_check(err != "", "load_song de una carpeta inexistente da error: " + err)
	eng.call("set_config", 50.0, 100.0, 1.5)
	eng.call("set_time_mapping", 0.0)
	var res: Array = eng.call("update", 1.0)
	_check(res.is_empty(), "update sin canción no da resultados")

	var none = _static("JDSource", "create_replay", ["/no/existe.jsonl", 1.0])
	_check(none == null, "create_replay de un archivo inexistente devuelve null")
	var phone: Object = _static("JDSource", "create_phone_server", [47811, 1234])
	_check(phone != null and phone.call("pairing_code") == 1234, "create_phone_server con código 1234")
	_check(str(phone.call("endpoint")).ends_with(":47811"), "endpoint " + str(phone.call("endpoint")))
	if phone.call("start"):
		var devs: Array = phone.call("devices")
		_check(devs.is_empty(), "sin móviles conectados")
		phone.call("stop")
	else:
		print("  aviso: no se pudo abrir el puerto UDP: ", phone.call("last_error"))

	if data_dir != "":
		var song: String = data_dir.path_join("sessions/synthetic/song_demo")
		var session: String = data_dir.path_join("sessions/synthetic/perfect.jsonl")
		_check(eng.call("load_song", song) == "", "load_song(song_demo)")
		var rep: Object = _static("JDSource", "create_replay", [session, 1.0])
		_check(rep != null, "create_replay(perfect.jsonl)")
		if rep != null:
			_check(rep.call("start"), "replay.start()")
			OS.delay_msec(200)
			var n: int = eng.call("pump", rep)
			_check(n > 0, "pump() movió %d muestras al motor" % n)
			var smp: Array = rep.call("poll_samples", 4)
			_check(smp.size() <= 4, "poll_samples(4) devuelve como mucho 4")
			if smp.size() > 0:
				_check(smp[0]["a"] is Vector3 and smp[0]["t_ns"] is int, "muestra con a:Vector3 y t_ns:int")
			rep.call("stop")
		# Grabador: envuelve un replay y escribe JSONL en user://.
		var rep2: Object = _static("JDSource", "create_replay", [session, 1.0])
		var out_path := OS.get_user_data_dir().path_join("ci_rec.jsonl")
		var rec: Object = _static("JDSource", "create_recorder", [rep2, out_path])
		_check(rec != null, "create_recorder(replay, %s)" % out_path)
		if rec != null:
			rec.call("set_recorder_mapping", 0.0)
			_check(rec.call("start"), "recorder.start()")
			OS.delay_msec(100)
			var m: Array = rec.call("poll_samples", 512)
			rec.call("stop")
			_check(m.size() > 0 and FileAccess.file_exists(out_path), "el grabador escribió %d muestras" % m.size())
		eng.call("reset")

	if _fails == 0:
		print("== GDExtension OK ==")
	quit(1 if _fails > 0 else 0)
