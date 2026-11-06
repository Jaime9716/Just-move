"""Validador Python de choreo.json: mismos casos que el cargador Rust (choreo_cases/cases.json)."""
import json
from pathlib import Path

import pytest

import choreo_validate as cv
from conftest import REPO, TOOLS

CASES = json.loads((Path(__file__).parent / "choreo_cases" / "cases.json").read_text(encoding="utf-8"))["cases"]


def errors_for(case):
    if "raw" in case:
        try:
            obj = cv.parse_text(case["raw"])
        except cv.ChoreoError as e:
            return e.errors
        return cv.validate(obj)
    return cv.validate(case["doc"])


@pytest.mark.parametrize("case", CASES, ids=[c["name"] for c in CASES])
def test_shared_cases(case):
    errs = errors_for(case)
    if case["error"] is None:
        assert errs == []
    else:
        assert errs, "debería fallar"
        assert any(case["error"] in e for e in errs), errs


def test_order_message_exact():
    doc = next(c for c in CASES if c["name"] == "unordered")["doc"]
    assert cv.validate(doc) == ["steps[2].t: debe ser >= steps[1].t (4.2 < 4.5)"]


def test_nan_rejected():
    with pytest.raises(cv.ChoreoError):
        cv.parse_text('{"x": NaN}')


def test_file_errors_prefixed(tmp_path):
    doc = next(c for c in CASES if c["name"] == "bad_kind")["doc"]
    p = tmp_path / "choreo.json"
    p.write_text(json.dumps(doc), encoding="utf-8")
    with pytest.raises(cv.ChoreoError) as ei:
        cv.load(p)
    assert ei.value.errors[0].startswith(f"{p}: steps[1].kind:")
    p.write_text("{", encoding="utf-8")
    with pytest.raises(cv.ChoreoError) as ei:
        cv.load(p)
    assert ei.value.errors[0].startswith(f"{p}:1:")


def test_save_roundtrip(tmp_path):
    doc = CASES[0]["doc"]
    p = tmp_path / "c.json"
    cv.save(p, doc)
    assert json.loads(p.read_text(encoding="utf-8")) == doc
    with pytest.raises(cv.ChoreoError):
        cv.save(p, {"format": "x"})


def test_cli(tmp_path, capsys):
    good = tmp_path / "good"
    good.mkdir()
    cv.save(good / "choreo.json", CASES[0]["doc"])
    bad = tmp_path / "bad.json"
    bad.write_text(json.dumps(next(c for c in CASES if c["name"] == "unknown_limb")["doc"]), encoding="utf-8")
    assert cv.main([str(good)]) == 0
    assert cv.main([str(good), str(bad)]) == 1
    assert "extremidad desconocida" in capsys.readouterr().err


def test_repo_packages_valid():
    for p in sorted((REPO / "data" / "songs").glob("*/choreo.json")):
        cv.load(p)
