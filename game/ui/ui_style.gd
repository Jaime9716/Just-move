# ui_style.gd — utilidades para construir la interfaz por código (agente E).
class_name UiStyle
extends RefCounted

const BG_COLOR := Color(0.06, 0.06, 0.10)
const ACCENT := Color(1.0, 0.45, 0.75)
const TEXT_DIM := Color(0.75, 0.75, 0.82)
const WARN := Color(1.0, 0.55, 0.25)
const OK_COLOR := Color(0.45, 0.95, 0.55)


static func label(text: String, font_size: int = 22, color: Color = Color.WHITE) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", font_size)
	l.add_theme_color_override("font_color", color)
	return l


static func button(text: String, font_size: int = 24, min_width: float = 0.0) -> Button:
	var b := Button.new()
	b.text = text
	b.add_theme_font_size_override("font_size", font_size)
	b.custom_minimum_size = Vector2(min_width, 48.0)
	return b


## Fondo a pantalla completa + MarginContainer + VBoxContainer. Devuelve el VBox.
static func page(root: Control, margin: int = 40, separation: int = 14) -> VBoxContainer:
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var bg := ColorRect.new()
	bg.color = BG_COLOR
	bg.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.add_child(bg)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var m := MarginContainer.new()
	root.add_child(m)
	m.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side: String in ["left", "right", "top", "bottom"]:
		m.add_theme_constant_override("margin_" + side, margin)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", separation)
	m.add_child(box)
	return box


static func hbox(separation: int = 12) -> HBoxContainer:
	var h := HBoxContainer.new()
	h.add_theme_constant_override("separation", separation)
	return h
