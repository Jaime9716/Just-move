"""calib_stats.py (P4), fps_log_stats.py (P9) y feedback_summary.py."""
import json

import numpy as np

from t3_helpers import ROOT, run_tool


# --- calib_stats ---------------------------------------------------------------------------------

def test_calib_numbers_pass_and_fail():
    p = run_tool("calib_stats.py", "32", "35", "29", "31", "34", "--json")
    r = json.loads(p.stdout)
    assert p.returncode == 0 and r["pass"] and r["range_ms"] == 6.0 and r["n"] == 5
    p = run_tool("calib_stats.py", "20", "+35", "29,5", "31", "34")
    assert p.returncode == 1 and "SUSPENDIDO" in p.stdout


def test_calib_csv_and_profiles(tmp_path):
    csv = tmp_path / "p4.csv"
    csv.write_text("intento;offset_ms;stddev_ms\n1;40;5\n2;42;4\n3;38;6\n4;41;5\n5;44;3\n", encoding="utf-8")
    r = json.loads(run_tool("calib_stats.py", csv, "--json").stdout)
    assert r["values_ms"] == [40, 42, 38, 41, 44] and r["pass"]
    txt = tmp_path / "p4.txt"
    txt.write_text("# ms\n10\n12\n", encoding="utf-8")
    r = json.loads(run_tool("calib_stats.py", txt, "--json").stdout)
    assert r["n"] == 2 and "warning" in r
    profs = []
    for i, v in enumerate([0.031, 0.036, 0.029]):
        f = tmp_path / f"profile{i}.cfg"
        f.write_text(json.dumps({"format": "jd-profile", "input_offset": v, "visual_offset": -0.01 * i}), encoding="utf-8")
        profs.append(f)
    r = json.loads(run_tool("calib_stats.py", *profs, "--json").stdout)
    assert np.allclose(r["values_ms"], [31, 36, 29]) and r["range_ms"] == 7.0
    r = json.loads(run_tool("calib_stats.py", *profs, "--key", "visual_offset", "--json").stdout)
    assert np.allclose(r["values_ms"], [0, -10, -20]) and not r["pass"]


def test_calib_bad_input():
    p = run_tool("calib_stats.py", "hola")
    assert p.returncode == 2


# --- fps_log_stats -------------------------------------------------------------------------------

def _fps_log(path, fps_values):
    t, lines = 0.0, ["t_s,fps"]
    for f in fps_values:
        t += 1.0 / f
        lines.append(f"{t:.4f},{f:.2f}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def test_fps_stable_passes(tmp_path):
    rng = np.random.default_rng(1)
    log = _fps_log(tmp_path / "fps.csv", 60 + rng.normal(0, 0.5, 60 * 120))
    p = run_tool("fps_log_stats.py", log, "--json")
    r = json.loads(p.stdout)
    assert p.returncode == 0 and r["pass"] and abs(r["fps_mean"] - 60) < 0.5 and r["stalls"] == 0


def test_fps_stalls_and_drops_fail(tmp_path):
    vals = np.full(60 * 60, 60.0)
    vals[1000] = 8.0          # tirón de 125 ms
    vals[2000:2100] = 40.0    # bajón
    log = _fps_log(tmp_path / "fps.csv", vals)
    r = json.loads(run_tool("fps_log_stats.py", log, "--json").stdout)
    assert not r["pass"] and r["stalls"] == 1 and r["frame_ms_max"] == 125.0
    assert r["frames_below_pct"] > 2
    assert r["worst_second"]["fps"] < 50
    p = run_tool("fps_log_stats.py", log)
    assert p.returncode == 1 and "tirón de 125.0 ms" in p.stdout


def test_fps_skip_warmup_and_target(tmp_path):
    vals = np.concatenate([np.full(30, 10.0), np.full(144 * 30, 144.0)])   # 3 s de carga lenta + 144 Hz
    log = _fps_log(tmp_path / "fps.csv", vals)
    r = json.loads(run_tool("fps_log_stats.py", log, "--skip-s", "4", "--json").stdout)
    assert r["pass"] and "warning" in r
    r = json.loads(run_tool("fps_log_stats.py", log, "--skip-s", "4", "--target", "144", "--json").stdout)
    assert r["pass"] and "warning" not in r
    r = json.loads(run_tool("fps_log_stats.py", log, "--skip-s", "0", "--json").stdout)
    assert not r["pass"]


def test_fps_bad_header(tmp_path):
    f = tmp_path / "x.csv"
    f.write_text("tiempo,valor\n1,2\n", encoding="utf-8")
    assert run_tool("fps_log_stats.py", f).returncode == 2


# --- feedback_summary ----------------------------------------------------------------------------

def test_feedback_summary_from_template(tmp_path):
    tpl = (ROOT / "docs/feedback/plantilla.md").read_text(encoding="utf-8")
    (tmp_path / "plantilla.md").write_text(tpl, encoding="utf-8")       # se ignora
    filled = (tpl.replace("<momento (Fase 0 / H0 / I1 / I2 / I3 / fallo)> · <AAAA-MM-DD>", "I1 · 2026-11-20")
              .replace("- Móvil (marca y modelo, versión de Android): <!-- p. ej. Xiaomi Redmi Note 12, Android 14 -->",
                       "- Móvil (marca y modelo, versión de Android): Pixel 7, Android 15")
              .replace("- Wi-Fi (router o compañía, banda 2,4 / 5 GHz, distancia al router, ¿PC por cable o Wi-Fi?):",
                       "- Wi-Fi (router o compañía, banda 2,4 / 5 GHz, distancia al router, ¿PC por cable o Wi-Fi?): 5 GHz, 3 m")
              .replace("| P? | | sí / no | |", "| P2 | 199,6 Hz, 0 huecos | sí | |\n| P3 | +14 ms al final | no | deriva |")
              .replace("- Diversión (1–5):", "- Diversión (1–5): 4")
              .replace("- Qué pasó:", "- Qué pasó: se desconectó al bloquear la pantalla"))
    (tmp_path / "2026-11-20-I1.md").write_text(filled, encoding="utf-8")
    later = filled.replace("I1 · 2026-11-20", "I2 · 2026-11-25").replace("| P3 | +14 ms al final | no | deriva |",
                                                                       "| P3 | +4 ms | sí | |").replace(": 4\n", ": 2\n")
    (tmp_path / "2026-11-25-I2.md").write_text(later, encoding="utf-8")
    p = run_tool("feedback_summary.py", "--dir", tmp_path, "--json")
    assert p.returncode == 0, p.stderr
    d = json.loads(p.stdout)
    s = d["summary"]
    assert s["files"] == 2
    assert s["latest"]["P3"]["pass"] is True and s["latest"]["P3"]["date"] == "2026-11-25"
    assert len(s["history"]["P3"]) == 2
    assert s["passed"] == ["P2", "P3"] and "P1" in s["pending"]
    assert s["fun_mean"] == 3.0
    assert s["phones"] == ["Pixel 7, Android 15"]
    assert len(s["failures"]) == 2
    first = d["items"][0]
    assert first["moment"] == "I1" and first["sections"]["Entorno"]["Móvil (marca y modelo, versión de Android)"]
    human = run_tool("feedback_summary.py", "--dir", tmp_path)
    assert "Pendientes" in human.stdout and "Pixel 7" in human.stdout


def test_feedback_summary_repo_dir_runs():
    assert run_tool("feedback_summary.py").returncode == 0
