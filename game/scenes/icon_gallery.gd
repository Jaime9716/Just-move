# icon_gallery.gd — muestra todos los pictogramas de StepIcons (agente E), más uno desconocido para
# ver el icono genérico de respaldo. Abrir con F6. Esc vuelve al menú.
extends Control

const UNKNOWN_ICON := "icono_que_no_existe"


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	resized.connect(queue_redraw)


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed(&"ui_cancel"):
		get_tree().change_scene_to_file(UiSession.MAIN_MENU_SCENE)


func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), UiStyle.BG_COLOR)
	var names := StepIcons.known_icons()
	names.append(UNKNOWN_ICON)
	var font := get_theme_default_font()
	var cols := 6
	var cell := Vector2(size.x / float(cols), 150.0)
	for i in range(names.size()):
		var c := Vector2((float(i % cols) + 0.5) * cell.x, (float(i / cols) + 0.5) * cell.y + 20.0)
		var limb := i % 4
		var grade := -1 if i < names.size() - 3 else (i % 3)
		StepIcons.draw_icon(self, names[i], c, 40.0, limb, grade)
		if font != null:
			draw_string(font, c + Vector2(-cell.x * 0.5, 66.0), names[i], HORIZONTAL_ALIGNMENT_CENTER, cell.x, 16, Color(1, 1, 1, 0.8))
