"""Estimación de BPM y offset (onset strength + autocorrelación)."""
import numpy as np
import pytest

import jd_media
from conftest import click_track


@pytest.mark.parametrize("bpm,offset", [(120, 1.0), (120, 0.37), (95, 0.2), (150, 2.0), (174, 0.5), (72, 0.8)])
def test_click_tracks(bpm, offset):
    x, sr = click_track(bpm, offset, 20.0, sr=jd_media.ANALYSIS_SR, seed=bpm)
    b, o = jd_media.estimate_tempo(x, sr)
    assert abs(b - bpm) <= 2.0
    assert abs(o - offset) <= 0.02


def test_offbeats_do_not_double_tempo():
    sr = jd_media.ANALYSIS_SR
    x, _ = click_track(100, 0.1, 20.0, sr=sr)
    y, _ = click_track(100, 0.1 + 0.3, 20.0, sr=sr, seed=5)  # contratiempos (más débiles)
    b, o = jd_media.estimate_tempo(x + 0.35 * y, sr)
    assert abs(b - 100) <= 2.0 and abs(o - 0.1) <= 0.02


def test_silence_defaults():
    assert jd_media.estimate_tempo(np.zeros(22050 * 5)) == (120.0, 0.0)


def test_skeleton_every_two_beats():
    s = jd_media.skeleton_steps(120.0, 0.5, 10.0)
    assert [x["t"] for x in s[:3]] == [0.5, 1.5, 2.5]
    assert s[0]["limbs"] == ["right_hand"] and s[1]["limbs"] == ["left_hand"]
    assert s[-1]["t"] + s[-1]["dur"] <= 9.0
