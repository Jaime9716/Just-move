# results.gd — resultados de la partida (agente E). Lee UiSession.last_result.
extends Control

var _session: UiSession


func _ready() -> void:
	_session = UiSession.get_state()
	var r := _session.last_result
	var box := UiStyle.page(self, 60, 16)
	box.add_child(UiStyle.label("Resultados", 44, UiStyle.ACCENT))
	if r.is_empty():
		box.add_child(UiStyle.label("No hay ninguna partida terminada.", 26))
	else:
		var counts: Dictionary = r.get("counts", {})
		var perfect := int(counts.get(2, 0))
		var good := int(counts.get(1, 0))
		var miss := int(counts.get(0, 0))
		var judged := maxi(int(r.get("judged", 0)), 1)
		box.add_child(UiStyle.label(str(r.get("title", "")), 30))
		box.add_child(UiStyle.label("%d puntos" % int(r.get("score", 0)), 64))
		box.add_child(UiStyle.label("Combo máximo: %d" % int(r.get("max_combo", 0)), 28))
		box.add_child(UiStyle.label("Perfect: %d   Good: %d   Miss: %d" % [perfect, good, miss], 28))
		box.add_child(UiStyle.label("Acierto: %.0f %%   (juzgados %d de %d)" % [
			100.0 * float(perfect + good) / float(judged), int(r.get("judged", 0)), int(r.get("expected", 0))], 22, UiStyle.TEXT_DIM))
		box.add_child(UiStyle.label(str(r.get("source", "")), 18, UiStyle.TEXT_DIM))
		box.add_child(UiStyle.label("Vídeo: %s · recolocaciones: %d" % [str(r.get("video_mode", "")), int(r.get("video_seeks", 0))], 16, UiStyle.TEXT_DIM))
	var spacer := Control.new()
	spacer.size_flags_vertical = Control.SIZE_EXPAND_FILL
	box.add_child(spacer)
	var row := UiStyle.hbox(16)
	box.add_child(row)
	var again := UiStyle.button("Repetir", 26, 200.0)
	again.disabled = _session.song.is_empty()
	again.pressed.connect(func() -> void: get_tree().change_scene_to_file(UiSession.GAMEPLAY_SCENE))
	row.add_child(again)
	var menu := UiStyle.button("Menú", 26, 200.0)
	menu.pressed.connect(_to_menu)
	row.add_child(menu)
	menu.grab_focus.call_deferred()


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed(&"ui_cancel"):
		_to_menu()


func _to_menu() -> void:
	get_tree().change_scene_to_file(UiSession.MAIN_MENU_SCENE)
