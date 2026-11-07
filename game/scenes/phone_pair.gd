# phone_pair.gd — emparejar móviles-mando (agente E).
# Abre (o reutiliza) el servidor de móviles compartido de UiSession (JDSource.create_phone_server con
# Profile.phone_port), muestra endpoint() y pairing_code() en grande y lista los móviles con batería,
# sync_rtt_ms (aviso si > 30 ms) y la extremidad de cada uno (se guarda en Profile.limb_by_device).
extends Control

const REFRESH_S := 0.25
const RTT_WARN_MS := 30.0

var _session: UiSession
var _server: Object = null
var _endpoint: Label
var _code: Label
var _error: Label
var _retry_btn: Button
var _devices_box: VBoxContainer
var _empty_label: Label
var _rows := {}          # id (int) → {name, battery, rtt, state, limb}
var _timer := 0.0


func _ready() -> void:
	_session = UiSession.get_state()
	_build_ui()
	_open_server()


func _process(delta: float) -> void:
	_timer -= delta
	if _timer <= 0.0:
		_timer = REFRESH_S
		_refresh()


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed(&"ui_cancel"):
		_back()


func _build_ui() -> void:
	var box := UiStyle.page(self)
	box.add_child(UiStyle.label("Conectar móvil", 40, UiStyle.ACCENT))
	box.add_child(UiStyle.label("Abre la app de mando en el móvil (misma red Wi-Fi que el PC) y escribe:", 22, UiStyle.TEXT_DIM))
	var big := UiStyle.hbox(60)
	box.add_child(big)
	var col1 := VBoxContainer.new()
	big.add_child(col1)
	col1.add_child(UiStyle.label("Dirección", 20, UiStyle.TEXT_DIM))
	_endpoint = UiStyle.label("—", 64)
	col1.add_child(_endpoint)
	var col2 := VBoxContainer.new()
	big.add_child(col2)
	col2.add_child(UiStyle.label("Código", 20, UiStyle.TEXT_DIM))
	_code = UiStyle.label("----", 64, UiStyle.ACCENT)
	col2.add_child(_code)

	_error = UiStyle.label("", 20, UiStyle.WARN)
	_error.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	box.add_child(_error)
	_retry_btn = UiStyle.button("Reintentar")
	_retry_btn.visible = false
	_retry_btn.pressed.connect(_open_server)
	box.add_child(_retry_btn)

	box.add_child(UiStyle.label("Móviles", 26))
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	box.add_child(scroll)
	_devices_box = VBoxContainer.new()
	_devices_box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_devices_box.add_theme_constant_override("separation", 8)
	scroll.add_child(_devices_box)
	_empty_label = UiStyle.label("Esperando móviles…", 22, UiStyle.TEXT_DIM)
	_devices_box.add_child(_empty_label)

	var row := UiStyle.hbox()
	box.add_child(row)
	var back := UiStyle.button("Volver", 24, 160.0)
	back.pressed.connect(_back)
	row.add_child(back)
	row.add_child(UiStyle.label("La extremidad elegida se recuerda por nombre de móvil.", 16, UiStyle.TEXT_DIM))


func _open_server() -> void:
	_server = _session.ensure_phone_server()
	if _server == null:
		_error.text = _session.phone_server_error + "\n¿Otro programa usa el puerto? Cambia phone_port en user://profile.cfg."
		_retry_btn.visible = true
		_endpoint.text = "—"
		_code.text = "----"
		return
	_error.text = ""
	_retry_btn.visible = false
	_endpoint.text = str(_server.call(&"endpoint"))
	_code.text = "%04d" % int(_server.call(&"pairing_code"))
	if EngineFactory.is_mock(_server):
		_error.text = "Simulación: sin GDExtension el servidor es falso (aparece \"Pixel (simulado)\")."
	_refresh()


func _refresh() -> void:
	if _server == null:
		return
	_session.apply_saved_limbs(_server)
	var devs := UiSession.devices_of(_server)
	var seen := {}
	for d: Variant in devs:
		if not (d is Dictionary):
			continue
		var dev: Dictionary = d
		var id := int(dev.get("id", 0))
		seen[id] = true
		if not _rows.has(id):
			_rows[id] = _make_row(id, str(dev.get("name", "?")))
		_update_row(_rows[id], dev)
	for id: Variant in _rows.keys():
		if not seen.has(id):
			var r: Dictionary = _rows[id]
			(r["root"] as Node).queue_free()
			_rows.erase(id)
	_empty_label.visible = _rows.is_empty()


func _make_row(id: int, dev_name: String) -> Dictionary:
	var panel := PanelContainer.new()
	_devices_box.add_child(panel)
	var h := UiStyle.hbox(24)
	panel.add_child(h)
	var name_l := UiStyle.label(dev_name, 24)
	name_l.custom_minimum_size = Vector2(320, 0)
	h.add_child(name_l)
	var state_l := UiStyle.label("", 18, UiStyle.TEXT_DIM)
	state_l.custom_minimum_size = Vector2(130, 0)
	h.add_child(state_l)
	var bat_l := UiStyle.label("", 20)
	bat_l.custom_minimum_size = Vector2(150, 0)
	h.add_child(bat_l)
	var rtt_l := UiStyle.label("", 20)
	rtt_l.custom_minimum_size = Vector2(300, 0)
	h.add_child(rtt_l)
	var opt := OptionButton.new()
	opt.add_theme_font_size_override("font_size", 20)
	for limb in range(UiSession.LIMB_LABELS.size()):
		opt.add_item(UiSession.limb_label(limb), limb)
	opt.item_selected.connect(_on_limb_selected.bind(id, dev_name))
	h.add_child(opt)
	return {"root": panel, "name": name_l, "state": state_l, "battery": bat_l, "rtt": rtt_l, "limb": opt}


func _update_row(row: Dictionary, dev: Dictionary) -> void:
	var connected := bool(dev.get("connected", false))
	var state_l: Label = row["state"]
	state_l.text = "conectado" if connected else "desconectado"
	state_l.add_theme_color_override("font_color", UiStyle.OK_COLOR if connected else UiStyle.WARN)

	var bat_l: Label = row["battery"]
	var b := float(dev.get("battery", -1.0))
	if b < 0.0:
		bat_l.text = "Batería —"
	else:
		var pct := b * 100.0 if b <= 1.0 else b   # 0–1 según el contrato; por si llegara en %
		bat_l.text = "Batería %d %%" % roundi(pct)
		bat_l.add_theme_color_override("font_color", UiStyle.WARN if pct < 20.0 else Color.WHITE)

	var rtt_l: Label = row["rtt"]
	var rtt := float(dev.get("sync_rtt_ms", 0.0))
	if rtt > RTT_WARN_MS:
		rtt_l.text = "RTT %.0f ms (!) Wi-Fi lenta" % rtt
		rtt_l.add_theme_color_override("font_color", UiStyle.WARN)
		rtt_l.tooltip_text = "Más de %.0f ms: acerca el PC al router o usa la banda de 5 GHz." % RTT_WARN_MS
	else:
		rtt_l.text = "RTT %.0f ms" % rtt
		rtt_l.add_theme_color_override("font_color", Color.WHITE)
		rtt_l.tooltip_text = ""

	var opt: OptionButton = row["limb"]
	if not opt.get_popup().visible:
		var limb := int(dev.get("limb", UiSession.LIMB_NONE))
		var idx := opt.get_item_index(limb) if limb >= 0 and limb < UiSession.LIMB_LABELS.size() else -1
		if opt.selected != idx:
			opt.select(idx)


func _on_limb_selected(index: int, id: int, dev_name: String) -> void:
	var opt_limb := index   # ids de los elementos = índice = limb
	_session.set_device_limb(_server, id, dev_name, opt_limb)


func _back() -> void:
	get_tree().change_scene_to_file(UiSession.MAIN_MENU_SCENE)
