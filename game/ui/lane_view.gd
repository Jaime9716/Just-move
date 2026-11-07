# lane_view.gd — carril de iconos (agente E). Un único _draw por fotograma para todos los iconos
# visibles (rápido: P9 pide 60 fps). Posiciones con LaneLogic a partir de SongClock.visual().
class_name LaneView
extends Control

## Orden de columnas en pantalla: mano izq, pie izq, pie der, mano der.
const COLUMN_OF_LIMB := {0: 0, 2: 1, 3: 2, 1: 3}
const LIMB_LABELS := {0: "MANO IZQ", 1: "MANO DER", 2: "PIE IZQ", 3: "PIE DER"}

var steps: Array = []
var step_times := PackedFloat64Array()
## índice de paso → peor grado recibido hasta ahora (para colorear el icono).
var step_grade: Dictionary = {}
var visual_t := 0.0
## Píxeles por segundo de caída.
var speed := 300.0
## Segundos que sigue visible un paso tras su instante.
var behind := 0.7
var icon_radius := 36.0
## Distancia del borde inferior a la línea de acierto.
var target_margin := 110.0
## Extremidades que tiene la canción (solo se dibujan esas columnas resaltadas).
var used_limbs: Dictionary = {}


func setup(p_steps: Array, p_times: PackedFloat64Array) -> void:
	steps = p_steps
	step_times = p_times
	step_grade = {}
	used_limbs = {}
	for st: Dictionary in steps:
		var limbs: PackedInt32Array = st["limbs"]
		for l: int in limbs:
			used_limbs[l] = true
	queue_redraw()


func target_y() -> float:
	return size.y - target_margin


## Centro x de la columna de una extremidad.
func column_x(limb: int) -> float:
	var col: int = COLUMN_OF_LIMB.get(limb, 0)
	return size.x * (float(col) + 0.5) / 4.0


func mark(step: int, grade: int) -> void:
	if step_grade.has(step):
		step_grade[step] = mini(int(step_grade[step]), grade)
	else:
		step_grade[step] = grade


func set_time(t: float) -> void:
	visual_t = t
	queue_redraw()


func _draw() -> void:
	var w := size.x
	var ty := target_y()
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.05, 0.05, 0.09, 0.82))
	var font := get_theme_default_font()
	for limb: int in [0, 1, 2, 3]:
		var x := column_x(limb)
		var active := used_limbs.has(limb)
		var col := StepIcons.limb_color(limb)
		col.a = 0.10 if active else 0.03
		draw_rect(Rect2(x - w / 8.0 + 3.0, 0.0, w / 4.0 - 6.0, size.y), col)
		# Diana en la línea de acierto.
		var ring := StepIcons.limb_color(limb)
		ring.a = 0.9 if active else 0.25
		draw_arc(Vector2(x, ty), icon_radius * 1.12, 0.0, TAU, 40, ring, 3.0, true)
		if font != null:
			var txt: String = LIMB_LABELS[limb]
			draw_string(font, Vector2(x - w / 8.0, size.y - 20.0), txt, HORIZONTAL_ALIGNMENT_CENTER, w / 4.0, 16, Color(1, 1, 1, 0.8 if active else 0.3))
	draw_line(Vector2(0, ty), Vector2(w, ty), Color(1, 1, 1, 0.35), 2.0)

	var ahead := (ty + icon_radius * 2.0) / maxf(speed, 1.0)
	var idx := LaneLogic.visible_steps(step_times, visual_t, ahead, behind)
	# Primero las barras de los pasos "move" (debajo de los iconos).
	for i: int in idx:
		var st: Dictionary = steps[i]
		if String(st["kind"]) != "move" or float(st["dur"]) <= 0.0:
			continue
		var y0 := LaneLogic.icon_y(float(st["t"]), visual_t, ty, speed)
		var y1 := LaneLogic.icon_y(float(st["t"]) + float(st["dur"]), visual_t, ty, speed)
		var limbs: PackedInt32Array = st["limbs"]
		for l: int in limbs:
			var c := StepIcons.limb_color(l)
			c.a = 0.45
			var x := column_x(l)
			draw_rect(Rect2(x - icon_radius * 0.35, y1, icon_radius * 0.7, y0 - y1), c)
	# Iconos (los más lejanos primero para que el próximo quede encima).
	for k in range(idx.size() - 1, -1, -1):
		var i: int = idx[k]
		var st: Dictionary = steps[i]
		var t := float(st["t"])
		var y := LaneLogic.icon_y(t, visual_t, ty, speed)
		var grade: int = int(step_grade.get(i, -1))
		var alpha := 1.0
		if visual_t > t:
			alpha = clampf(1.0 - (visual_t - t) / behind, 0.15, 1.0)
		var limbs: PackedInt32Array = st["limbs"]
		for l: int in limbs:
			StepIcons.draw_icon(self, String(st["icon"]), Vector2(column_x(l), y), icon_radius, l, grade, alpha)
