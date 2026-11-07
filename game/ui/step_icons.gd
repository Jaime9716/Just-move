# step_icons.gd — pictogramas de pasos dibujados por código (agente E). Sin imágenes ni nada con
# copyright: solo primitivas de CanvasItem. Icono desconocido → icono genérico.
class_name StepIcons
extends RefCounted

## Colores por extremidad (JD_Limb: 0 mano izq, 1 mano der, 2 pie izq, 3 pie der).
const LIMB_COLORS := [
	Color(0.20, 0.55, 1.00),
	Color(1.00, 0.35, 0.30),
	Color(0.25, 0.85, 0.45),
	Color(1.00, 0.80, 0.20),
]
const GRADE_COLORS := {0: Color(0.55, 0.55, 0.6), 1: Color(0.3, 0.9, 1.0), 2: Color(1.0, 0.85, 0.2)}

## Nombre de icono (minúsculas) → dibujo. Los no listados usan "generic".
const ALIASES := {
	"punch": "punch_forward", "punch_front": "punch_forward", "push": "punch_forward",
	"punch_r": "punch_right", "punch_l": "punch_left", "point_up": "punch_up", "raise": "punch_up",
	"punch_d": "punch_down", "wave_left": "wave", "wave_right": "wave", "waves": "wave",
	"stomp_l": "stomp_left", "stomp_r": "stomp_right", "kick": "stomp", "step": "stomp",
	"circle": "arm_circle", "roll": "arm_circle", "spin": "arm_circle", "rotate": "arm_circle",
	"clap_hands": "clap", "shake": "shake", "jump": "jump", "hop": "jump",
	"side": "side_step", "slide": "side_step", "freeze": "pose", "star": "pose",
	"pump_up": "pump", "swing_arm": "swing", "edit_me": "generic", "": "generic",
}
const KNOWN := [
	"punch_up", "punch_down", "punch_left", "punch_right", "punch_forward", "wave", "stomp",
	"stomp_left", "stomp_right", "clap", "arm_circle", "swing", "pump", "side_step", "pose",
	"shake", "jump", "generic",
]


static func known_icons() -> PackedStringArray:
	return PackedStringArray(KNOWN)


## Nombre de dibujo para un texto de icono libre de choreo.json.
static func resolve(icon: String) -> String:
	var k := icon.strip_edges().to_lower().replace("-", "_").replace(" ", "_")
	if KNOWN.has(k):
		return k
	if ALIASES.has(k):
		return String(ALIASES[k])
	return "generic"


static func limb_color(limb: int) -> Color:
	if limb >= 0 and limb < LIMB_COLORS.size():
		return LIMB_COLORS[limb]
	return Color(0.8, 0.8, 0.8)


## Dibuja el icono centrado en `c` con radio `r`. `grade` −1 = aún sin juzgar.
static func draw_icon(ci: CanvasItem, icon: String, c: Vector2, r: float, limb: int, grade: int = -1, alpha: float = 1.0) -> void:
	var base := limb_color(limb)
	base.a = alpha
	var fg := Color(1, 1, 1, alpha)
	if grade >= 0:
		var gc: Color = GRADE_COLORS.get(grade, Color.WHITE)
		gc.a = alpha
		ci.draw_circle(c, r * 1.18, gc)
		if grade == 0:
			base = base.darkened(0.6)
			fg = Color(0.75, 0.75, 0.75, alpha)
	ci.draw_circle(c, r, base)
	ci.draw_arc(c, r, 0.0, TAU, 40, Color(1, 1, 1, 0.85 * alpha), maxf(2.0, r * 0.06), true)
	var w := maxf(2.0, r * 0.13)
	match resolve(icon):
		"punch_up":
			_punch(ci, c, r, Vector2.UP, fg, w)
		"punch_down":
			_punch(ci, c, r, Vector2.DOWN, fg, w)
		"punch_left":
			_punch(ci, c, r, Vector2.LEFT, fg, w)
		"punch_right":
			_punch(ci, c, r, Vector2.RIGHT, fg, w)
		"punch_forward":
			_burst(ci, c, r, fg, w)
		"wave":
			_wave(ci, c, r, fg, w)
		"stomp":
			_stomp(ci, c, r, fg, w, 0.0)
		"stomp_left":
			_stomp(ci, c, r, fg, w, -1.0)
		"stomp_right":
			_stomp(ci, c, r, fg, w, 1.0)
		"clap":
			_clap(ci, c, r, fg, w)
		"arm_circle":
			_circle_arrow(ci, c, r, fg, w)
		"swing":
			_swing(ci, c, r, fg, w)
		"pump":
			_pump(ci, c, r, fg, w)
		"side_step":
			_side_step(ci, c, r, fg, w)
		"pose":
			_star(ci, c, r * 0.6, fg)
		"shake":
			_shake(ci, c, r, fg, w)
		"jump":
			_jump(ci, c, r, fg, w)
		_:
			_generic(ci, c, r, fg, w)


# ------------------------------------------------------------------ primitivas

static func _arrow(ci: CanvasItem, a: Vector2, b: Vector2, col: Color, w: float) -> void:
	var d := b - a
	if d.length() < 0.001:
		return
	var dir := d.normalized()
	var n := Vector2(-dir.y, dir.x)
	var head := w * 1.6
	var base := b - dir * head * 1.7
	ci.draw_line(a, base, col, w, true)
	ci.draw_colored_polygon(PackedVector2Array([b, base + n * head, base - n * head]), col)


static func _ellipse(c: Vector2, rx: float, ry: float, rot: float = 0.0, n: int = 24) -> PackedVector2Array:
	var pts := PackedVector2Array()
	for i in range(n):
		var a := TAU * float(i) / float(n)
		pts.append(c + Vector2(cos(a) * rx, sin(a) * ry).rotated(rot))
	return pts


static func _punch(ci: CanvasItem, c: Vector2, r: float, dir: Vector2, col: Color, w: float) -> void:
	var fist := c - dir * r * 0.35
	ci.draw_circle(fist, r * 0.24, col)
	_arrow(ci, fist + dir * r * 0.2, c + dir * r * 0.72, col, w)
	var n := Vector2(-dir.y, dir.x)
	for s: float in [-1.0, 1.0]:
		var p := fist + n * s * r * 0.38
		ci.draw_line(p - dir * r * 0.15, p + dir * r * 0.2, Color(col, col.a * 0.7), w * 0.5, true)


static func _burst(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	ci.draw_circle(c, r * 0.26, col)
	for i in range(8):
		var dir := Vector2.RIGHT.rotated(TAU * float(i) / 8.0)
		ci.draw_line(c + dir * r * 0.42, c + dir * r * 0.72, col, w * 0.7, true)


static func _wave(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	var pts := PackedVector2Array()
	for i in range(25):
		var x := -0.7 + 1.4 * float(i) / 24.0
		pts.append(c + Vector2(x * r, -sin(x * PI * 2.2) * r * 0.28))
	ci.draw_polyline(pts, col, w, true)
	ci.draw_circle(c + Vector2(0.0, r * 0.45), r * 0.12, col)


static func _foot(ci: CanvasItem, c: Vector2, r: float, col: Color) -> void:
	ci.draw_colored_polygon(_ellipse(c, r * 0.22, r * 0.36, 0.25), col)
	for i in range(4):
		ci.draw_circle(c + Vector2(-r * 0.15 + r * 0.11 * float(i), -r * 0.45 + absf(float(i) - 1.5) * r * 0.04), r * 0.055, col)


static func _stomp(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float, side: float) -> void:
	var fc := c + Vector2(side * r * 0.12, -r * 0.1)
	_foot(ci, fc, r, col)
	var y := c.y + r * 0.42
	ci.draw_line(Vector2(c.x - r * 0.55, y), Vector2(c.x + r * 0.55, y), col, w * 0.6, true)
	for s: float in [-1.0, 1.0]:
		ci.draw_line(Vector2(c.x + s * r * 0.38, y - r * 0.05), Vector2(c.x + s * r * 0.6, y - r * 0.25), col, w * 0.5, true)
	if side != 0.0:
		_arrow(ci, c + Vector2(-side * r * 0.1, r * 0.62), c + Vector2(side * r * 0.5, r * 0.62), col, w * 0.45)


static func _clap(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	ci.draw_colored_polygon(_ellipse(c + Vector2(-r * 0.16, r * 0.08), r * 0.17, r * 0.36, -0.35), col)
	ci.draw_colored_polygon(_ellipse(c + Vector2(r * 0.16, r * 0.08), r * 0.17, r * 0.36, 0.35), col)
	for a: float in [-2.2, -1.57, -0.94]:
		var dir := Vector2.RIGHT.rotated(a)
		ci.draw_line(c + Vector2(0, -r * 0.3) + dir * r * 0.18, c + Vector2(0, -r * 0.3) + dir * r * 0.4, col, w * 0.5, true)


static func _circle_arrow(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	var rr := r * 0.5
	var a0 := -PI * 0.35
	var a1 := a0 + TAU * 0.82
	ci.draw_arc(c, rr, a0, a1, 32, col, w, true)
	var tip := c + Vector2(cos(a1), sin(a1)) * rr
	var tangent := Vector2(-sin(a1), cos(a1))
	_arrow(ci, tip - tangent * r * 0.05, tip + tangent * r * 0.22, col, w)


static func _swing(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	var pivot := c + Vector2(0, -r * 0.45)
	var rr := r * 0.85
	ci.draw_arc(pivot, rr, PI * 0.28, PI * 0.72, 20, col, w, true)
	ci.draw_line(pivot, pivot + Vector2(cos(PI * 0.5), sin(PI * 0.5)) * rr, Color(col, col.a * 0.6), w * 0.5, true)
	ci.draw_circle(pivot + Vector2(0, rr), r * 0.13, col)
	for a: float in [PI * 0.28, PI * 0.72]:
		var tip := pivot + Vector2(cos(a), sin(a)) * rr
		var tangent := Vector2(-sin(a), cos(a)) * (-1.0 if a < PI * 0.5 else 1.0)
		_arrow(ci, tip - tangent * r * 0.02, tip + tangent * r * 0.18, col, w * 0.8)


static func _pump(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	for k in range(2):
		var y := c.y + r * 0.25 - float(k) * r * 0.42
		ci.draw_polyline(PackedVector2Array([
			Vector2(c.x - r * 0.42, y + r * 0.2), Vector2(c.x, y - r * 0.15), Vector2(c.x + r * 0.42, y + r * 0.2),
		]), col, w, true)


static func _side_step(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	_arrow(ci, c, c + Vector2(r * 0.72, 0), col, w)
	_arrow(ci, c, c - Vector2(r * 0.72, 0), col, w)
	ci.draw_colored_polygon(_ellipse(c + Vector2(0, -r * 0.38), r * 0.12, r * 0.2), col)
	ci.draw_colored_polygon(_ellipse(c + Vector2(0, r * 0.38), r * 0.12, r * 0.2), col)


static func _star(ci: CanvasItem, c: Vector2, rr: float, col: Color) -> void:
	var pts := PackedVector2Array()
	for i in range(10):
		var a := -PI * 0.5 + TAU * float(i) / 10.0
		var rad := rr if i % 2 == 0 else rr * 0.45
		pts.append(c + Vector2(cos(a), sin(a)) * rad)
	ci.draw_colored_polygon(pts, col)


static func _shake(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	var pts := PackedVector2Array()
	for i in range(7):
		var x := -0.6 + 1.2 * float(i) / 6.0
		pts.append(c + Vector2(x * r, (r * 0.25) * (1.0 if i % 2 == 0 else -1.0)))
	ci.draw_polyline(pts, col, w, true)


static func _jump(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	_arrow(ci, c + Vector2(0, r * 0.35), c + Vector2(0, -r * 0.7), col, w)
	ci.draw_line(c + Vector2(-r * 0.55, r * 0.55), c + Vector2(r * 0.55, r * 0.55), col, w * 0.7, true)


static func _generic(ci: CanvasItem, c: Vector2, r: float, col: Color, w: float) -> void:
	var d := r * 0.5
	ci.draw_polyline(PackedVector2Array([
		c + Vector2(0, -d), c + Vector2(d, 0), c + Vector2(0, d), c + Vector2(-d, 0), c + Vector2(0, -d),
	]), col, w, true)
	ci.draw_circle(c, r * 0.12, col)
