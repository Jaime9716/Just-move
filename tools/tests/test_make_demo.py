"""Paquete demo_metronomo: medios válidos y choreo.json determinista (igual al versionado)."""
import json
import subprocess

import choreo_validate as cv
import jd_media
import make_demo_song
from conftest import REPO, needs_ffmpeg


def test_choreo_matches_committed(tmp_path):
    assert make_demo_song.main(["--out", str(tmp_path), "--choreo-only"]) == 0
    gen = (tmp_path / "demo_metronomo" / "choreo.json").read_text(encoding="utf-8")
    committed = REPO / "data" / "songs" / "demo_metronomo" / "choreo.json"
    assert committed.exists(), "falta data/songs/demo_metronomo/choreo.json (ejecuta tools/make_demo_song.py)"
    assert gen == committed.read_text(encoding="utf-8")
    c = cv.load(committed)
    kinds = {s["kind"] for s in c["steps"]}
    limbs = {l for s in c["steps"] for l in s["limbs"]}
    assert kinds == {"accent", "move"} and limbs == set(cv.LIMBS)


@needs_ffmpeg
def test_media(tmp_path):
    assert make_demo_song.main(["--out", str(tmp_path), "--seconds", "12"]) == 0
    pkg = tmp_path / "demo_metronomo"
    ff, fp = jd_media.require_ffmpeg()
    a = jd_media.probe(fp, pkg / "audio.ogg")["streams"]
    assert [(s["codec_name"], int(s["sample_rate"]), s["channels"]) for s in a] == [("vorbis", 44100, 2)]
    v = jd_media.probe(fp, pkg / "video.ogv")["streams"]
    assert [s["codec_type"] for s in v] == ["video"] and v[0]["codec_name"] == "theora"
    assert int(v[0]["height"]) <= 720 and v[0]["r_frame_rate"] == "30/1"
    bpm, off = jd_media.estimate_tempo(jd_media.decode_mono(ff, pkg / "audio.ogg"))
    assert abs(bpm - 120) <= 2 and abs(off - 1.0) <= 0.03
    meta = json.loads((pkg / "meta.json").read_text(encoding="utf-8"))
    assert meta["source"] == "generated"
    cv.load(pkg / "choreo.json")
