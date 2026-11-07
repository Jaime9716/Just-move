"""av_sync_from_video.py con vídeos sintéticos generados con ffmpeg (desfases conocidos)."""
import json

import pytest

import av_sync_from_video as av
from t3_helpers import make_av_video, run_tool

TOL_MS = 3.0   # 240 fps → fotograma de 4,2 ms; la interpolación y la mediana dan < 3 ms


@pytest.mark.parametrize("offset_ms", [30.0, -20.0])
def test_known_offsets_h264(tmp_path, offset_ms):
    v = make_av_video(tmp_path / f"o{offset_ms:+.0f}.mp4", offset_s=offset_ms / 1e3)
    r = av.analyze(str(v))
    assert r["pairs"] >= 30
    assert abs(r["offset_start_ms"] - offset_ms) < TOL_MS
    assert abs(r["offset_end_ms"] - offset_ms) < TOL_MS
    assert abs(r["drift_ms_per_min"]) < 5
    assert r["pass"] is False          # 30 y 20 ms superan la tolerancia de 10 ms


def test_small_offset_passes_and_cli_json(tmp_path):
    v = make_av_video(tmp_path / "o5.mkv", offset_s=0.005, codec="ffv1")
    p = run_tool("av_sync_from_video.py", v, "--json")
    assert p.returncode == 0, p.stderr
    r = json.loads(p.stdout)
    assert r["pass"] is True
    assert abs(r["offset_median_ms"] - 5.0) < TOL_MS
    assert "pairs_detail" not in r


def test_drift_start_vs_end(tmp_path):
    # +0 ms al principio y +1 ms por segundo → ≈ +18 ms al final de 20 s
    v = make_av_video(tmp_path / "drift.mkv", offset_s=0.0, drift_s_per_s=0.001, codec="ffv1")
    r = av.analyze(str(v))
    assert abs(r["offset_start_ms"]) < 5
    assert r["offset_end_ms"] > 12
    assert 40 < r["drift_ms_per_min"] < 80      # 1 ms/s = 60 ms/min
    assert r["pass"] is False


def test_slowdown_factor(tmp_path):
    # cámara lenta ×8 guardada a 30 fps: en el archivo el desfase es 8 × 30 ms
    v = make_av_video(tmp_path / "slow.mkv", offset_s=0.030, seconds=8, fps=30, slowdown=8, codec="ffv1")
    r = av.analyze(str(v), slowdown=8)
    assert abs(r["offset_median_ms"] - 30.0) < TOL_MS
    assert abs(r["real_fps"] - 240) < 5


def test_camera_offset_and_roi(tmp_path):
    v = make_av_video(tmp_path / "cam.mkv", offset_s=0.030, seconds=8, codec="ffv1")
    r = av.analyze(str(v), camera_offset_ms=25.0, roi=(0.25, 0.25, 0.5, 0.5))
    assert abs(r["offset_median_ms"] - 5.0) < TOL_MS
    assert r["pass"] is True


def test_errors(tmp_path):
    p = run_tool("av_sync_from_video.py", tmp_path / "no_existe.mp4")
    assert p.returncode == 2 and "ERROR" in p.stderr
    p = run_tool("av_sync_from_video.py", "--help")
    assert p.returncode == 0 and "--slowdown" in p.stdout
