# requires: VideoSyncLogic
# Tests de caja negra de VideoSyncLogic.correction (contracts/game_logic_api.md).
extends "res://tests/test_base.gd"


func _check(r: Dictionary, seek: bool, target: float, msg: String) -> void:
	assert_has(r, "seek", msg)
	assert_has(r, "target", msg)
	assert_eq(r.get("seek"), seek, msg + ": seek")
	assert_near(float(r.get("target", -999.0)), target, 1e-12, msg + ": target")


func test_default_threshold_is_0_04() -> void:
	# Umbral por defecto del contrato: 0,04 s.
	_check(VideoSyncLogic.correction(10.0, 10.03), false, 10.03, "30 ms de deriva")
	_check(VideoSyncLogic.correction(10.0, 10.039), false, 10.039, "39 ms")
	_check(VideoSyncLogic.correction(10.0, 10.041), true, 10.041, "41 ms")
	_check(VideoSyncLogic.correction(10.0, 10.5), true, 10.5, "vídeo atrasado 0,5 s")
	_check(VideoSyncLogic.correction(10.5, 10.0), true, 10.0, "vídeo adelantado 0,5 s")
	_check(VideoSyncLogic.correction(10.041, 10.0), true, 10.0, "vídeo adelantado 41 ms")
	_check(VideoSyncLogic.correction(10.0, 10.0), false, 10.0, "sin deriva")
	done()


func test_explicit_threshold() -> void:
	_check(VideoSyncLogic.correction(1.0, 1.05, 0.07), false, 1.05, "50 ms con umbral 0,07")
	_check(VideoSyncLogic.correction(1.0, 1.08, 0.07), true, 1.08, "80 ms con umbral 0,07")
	_check(VideoSyncLogic.correction(1.0, 1.01, 0.005), true, 1.01, "10 ms con umbral 0,005")
	_check(VideoSyncLogic.correction(1.0, 0.92, 0.07), true, 0.92, "−80 ms con umbral 0,07")
	done()


func test_target_never_negative() -> void:
	_check(VideoSyncLogic.correction(0.5, -0.2), true, 0.0, "visual negativo (cuenta atrás) → target 0")
	_check(VideoSyncLogic.correction(3.0, -1.0, 0.07), true, 0.0, "umbral explícito → target 0")
	var r := VideoSyncLogic.correction(0.0, -0.01)
	assert_ge(float(r["target"]), 0.0, "target ≥ 0 aunque no haya seek")
	done()


func test_target_is_visual_t() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 3
	var bad := 0
	for i in range(500):
		var vt := rng.randf_range(0.0, 200.0)
		var vp := vt + rng.randf_range(-1.0, 1.0)
		var r := VideoSyncLogic.correction(vp, vt)
		var want_seek := absf(vp - vt) > 0.04
		if bool(r["seek"]) != want_seek and absf(absf(vp - vt) - 0.04) > 1e-9:
			bad += 1
		if absf(float(r["target"]) - vt) > 1e-12:
			bad += 1
	assert_eq(bad, 0, "casos aleatorios que no cumplen el contrato")
	done()
