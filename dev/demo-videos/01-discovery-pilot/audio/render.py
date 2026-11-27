#!/usr/bin/env python3
"""Render Quiet Index with Python 3, NumPy, and FFmpeg's native AAC encoder."""

import argparse
import json
import math
from pathlib import Path
import subprocess
import tempfile

import numpy as np


RATE = 48_000
TAU = 2 * math.pi
TITLE = "Quiet Index"

# Each harmony is a bass note and four pad voices, low to high, in MIDI pitches.
HARMONIES = {
    "D9": (38, 57, 61, 64, 66),
    "A6/C#": (37, 57, 59, 64, 66),
    "Bm7": (35, 54, 57, 62, 69),
    "G9": (43, 54, 57, 59, 62),
    "Em9": (40, 55, 59, 62, 66),
    "A": (45, 52, 57, 61, 66),
    "D/F#": (42, 57, 62, 64, 69),
    "G6": (43, 55, 59, 62, 64),
    "F#m7": (42, 52, 57, 61, 69),
    "Asus": (45, 55, 59, 62, 64),
    "D6": (38, 54, 57, 59, 62),
}

# The low tine plays two-bar calls and answers. Each stroke gives the eighth
# within the bar, the chord voice (raised bass or one of the two low pad
# voices), and its velocity. The syncopated call resolves to longer spaces.
CALL = ((0, 0, 1.00), (3, 2, 0.64), (6, 1, 0.76))
ANSWER = ((0, 0, 0.92), (4, 2, 0.68))
DOWN = ((0, 0, 0.80),)
REST = ()

# Each cell holds a chord, the high tine melody in eighth notes, and one
# low-tine part per bar. The check results and size summary have open bars.
SCORE = (
    ("D9", (), REST, ANSWER),
    ("A6/C#", ((3, 69), (6, 66), (12, 64)), CALL, DOWN),
    ("Bm7", ((5, 62),), REST, REST),
    ("G9", ((2, 66), (9, 69)), CALL, ANSWER),
    ("D/F#", ((3, 69), (6, 66), (13, 64)), CALL, DOWN),
    ("Em9", ((6, 67), (11, 66)), REST, ANSWER),
    ("G6", ((3, 64),), DOWN, CALL),
    ("A", (), CALL, ANSWER),

    ("Bm7", ((1, 66), (5, 69), (11, 73)), REST, DOWN),
    ("F#m7", ((4, 71), (10, 69)), CALL, ANSWER),
    ("G9", ((2, 66), (8, 62)), CALL, DOWN),
    ("D/F#", ((7, 64),), REST, ANSWER),
    ("Em9", ((1, 67), (5, 71), (11, 74)), DOWN, ANSWER),
    ("G6", ((4, 71), (9, 69)), REST, CALL),
    ("Asus", ((3, 67), (12, 64)), ANSWER, REST),
    ("A", (), REST, CALL),

    ("G9", ((5, 62), (11, 66)), CALL, ANSWER),
    ("D/F#", ((4, 69),), CALL, DOWN),
    ("Em9", ((3, 67), (7, 66), (13, 62)), REST, REST),
    # This B minor downbeat introduces the install-list chapter.
    ("Bm7", (), CALL, ANSWER),
    ("G6", ((2, 64), (9, 67)), CALL, ANSWER),
    ("F#m7", ((3, 69), (9, 64)), CALL, ANSWER),
    ("Em9", ((6, 62),), DOWN, CALL),
    ("Asus", ((3, 64), (11, 67)), CALL, ANSWER),

    ("D9", ((2, 69), (6, 66), (12, 64)), ANSWER, REST),
    ("A6/C#", ((5, 61),), CALL, ANSWER),
    ("Bm7", ((3, 62), (8, 66)), DOWN, REST),
    ("G9", ((6, 69),), REST, CALL),
    ("Em9", ((4, 67), (10, 64)), ANSWER, REST),
    ("Asus", ((4, 62),), REST, REST),
    ("A", ((3, 64), (10, 61)), ANSWER, REST),
    ("D6", ((1, 62),), REST, REST),
)


def frequency(note):
    return 440 * 2 ** ((note - 69) / 12)


def pan(signal, position):
    angle = (position + 1) * math.pi / 4
    return signal[:, None] * np.array([math.cos(angle), math.sin(angle)])


def rise(progress):
    """Raised cosine with zero slope at both ends of an attack or release."""
    return 0.5 - 0.5 * np.cos(math.pi * np.clip(progress, 0, 1))


def chord(t, length, tail, name, cell):
    bass, *voices = HARMONIES[name]
    release = rise((length + tail - t) / tail)
    bass_phase = TAU * frequency(bass) * t
    bass_tone = np.sin(bass_phase) + 0.28 * np.sin(2 * bass_phase)
    stereo = pan(0.64 * bass_tone * rise(t / min(0.46, length / 4)), 0)
    for part, (note, position) in enumerate(zip(voices, (-0.48, 0.31, -0.24, 0.53))):
        start = (part * 0.07 + 0.03) * min(1, length / 3)
        attack = rise((t - start) / min(1.45 + 0.12 * part, length / 2))
        phase = TAU * frequency(note) * t + part * 0.8
        motion = np.sin(0.72 * t + 0.67 * cell + part)
        tone = np.sin(phase) + (0.23 + 0.06 * motion) * np.sin(2 * phase)
        tone += 0.13 * np.sin(3 * phase) + 0.055 * np.sin(5 * phase)
        # A quieter detuned rank across the stereo field supplies the slow beating.
        second = np.sin(phase * 2 ** ((-1) ** part * 3 / 1200) + 0.9)
        stereo += 0.31 * attack[:, None] * (pan(tone, position) + 0.4 * pan(second, -position))
    return stereo * release[:, None]


def tine(t, note, damping):
    """The lead and muted low notes share a tine with short-lived overtones."""
    phase = TAU * frequency(note) * t
    decay_time = damping * t
    body = np.sin(phase + 0.52 * np.exp(-3.2 * decay_time) * np.sin(phase * 2))
    return body + 0.27 * np.exp(-2.4 * decay_time) * np.sin(phase * 2.003)


def add_pulse(stereo, length, name, bars):
    bass, low, middle, *_ = HARMONIES[name]
    pitches = (bass + 12, low, middle)
    eighth = length / (8 * len(bars))
    for bar, strokes in enumerate(bars):
        for beat, voice, velocity in strokes:
            start = round((8 * bar + beat) * eighth * RATE)
            # Allow the last note to ring over the next downbeat in the
            # shared overlap. Scale its envelope for abbreviated edits.
            duration = min(1.45, 3.7 * eighth)
            count = min(round(duration * RATE), len(stereo) - start)
            t = np.arange(count) / RATE
            age = t / duration
            envelope = rise(age / 0.025) * np.exp(-5.1 * age)
            envelope *= rise((1 - age) / 0.18)
            sound = 0.75 * velocity * envelope * tine(t, pitches[voice], damping=2.4)
            stereo[start:start + count] += pan(sound, (0, -0.14, 0.14)[voice])


def add_figure(stereo, length, notes, cell):
    for index, (eighth, note) in enumerate(notes):
        start = round(RATE * length * eighth / 16)
        count = min(len(stereo) - start, round(RATE * 3.35))
        t = np.arange(count) / RATE
        body = tine(t, note, damping=1)
        note_env = rise(t / 0.064) * np.exp(-1.64 * t) * rise((count / RATE - t) / 0.22)
        position = 0.16 * math.sin(cell + index * 1.2)
        dry = (0.52 if index % 2 == 0 else 0.43) * body * note_env
        for delay, level, place in ((0, 1, position), (0.19, 0.16, -0.54), (0.327, 0.09, 0.45)):
            offset = start + round(delay * RATE)
            size = min(count, len(stereo) - offset)
            if size > 0:
                # The local buffer may end inside a late reflection for short edits.
                edge = rise((len(stereo) - offset - np.arange(size)) / (0.04 * RATE))
                stereo[offset:offset + size] += pan(dry[:size] * edge * level, place)


def score_boundaries(story, duration):
    """Fit whole bars between the install-list downbeat and the final tonic."""
    times = {}
    end = 0
    for step in story["steps"]:
        times[step["scene"]] = end
        end += step["duration"]
    scale = RATE * (duration if duration is not None else end) / end
    # Extend the title phrases locally. Returning to tempo on the first typed
    # letter and on the official package selection keeps the old tutorial pulse.
    anchors = [(0, 0), (1, times["search-ripgrep"] + 1.15),
               (19, times["build-install-list"]), (20, times["queue-choice"] + 2.9),
               (len(SCORE) - 1, times["end-credits"]), (len(SCORE), end)]
    boundaries = []
    for (first, start), (last, stop) in zip(anchors, anchors[1:]):
        boundaries.extend(round(scale * (start + (stop - start) * (i - first) / (last - first)))
                          for i in range(first, last))
    return [*boundaries, round(scale * end)]


def blocks(boundaries):
    overlap = min(round(RATE * 1.55), min(np.diff(boundaries)) // 2)
    carry = np.zeros((overlap, 2))
    for cell, (name, notes, first_bar, second_bar) in enumerate(SCORE):
        start, end = boundaries[cell:cell + 2]
        count = end - start
        length = count / RATE
        t = np.arange(count + overlap) / RATE
        stereo = chord(t, length, overlap / RATE, name, cell)
        add_figure(stereo, length, notes, cell)
        bars = (first_bar, second_bar)
        if cell == 0:
            bars = (REST, *bars)
        elif cell == 19:
            bars = (first_bar, REST, first_bar, second_bar)
        add_pulse(stereo, length, name, bars)
        stereo[:overlap] += carry
        carry = stereo[count:]
        output = stereo[:count]
        if cell == len(SCORE) - 1:
            # Leave 50 ms of digital silence after the release for AAC padding.
            silence = min(0.05, length / 50)
            output = output * rise((length - silence - t[:count]) / (length - silence))[:, None]
        yield (output * 0.19).astype("<f4")


def render(duration, output):
    story = json.loads((Path(__file__).resolve().parents[1] / "storyboard.json").read_text())
    boundaries = score_boundaries(story, duration)
    command = [
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
        "-f", "f32le", "-ar", str(RATE), "-ac", "2", "-i", "pipe:0",
        "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart",
        "-movie_timescale", str(RATE), "-metadata", f"title={TITLE}",
    ]
    with tempfile.TemporaryDirectory(dir=output.parent) as directory:
        encoded = Path(directory) / "score.m4a"
        with subprocess.Popen([*command, str(encoded)], stdin=subprocess.PIPE) as encoder:
            with encoder.stdin as stream:
                for block in blocks(boundaries):
                    stream.write(block.tobytes())
            if encoder.wait():
                raise RuntimeError("FFmpeg failed to encode the score")
        encoded.replace(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--duration", type=float, metavar="SECONDS",
                        help="scale the score to this many seconds (default: storyboard duration)")
    parser.add_argument("--output", type=Path, default=Path(__file__).with_name("quiet-index.m4a"))
    args = parser.parse_args()
    if args.duration is not None and (not math.isfinite(args.duration) or args.duration < 1):
        parser.error("--duration must be a finite number of at least one second")
    if args.output.suffix.lower() != ".m4a":
        parser.error("--output must end in .m4a")
    render(args.duration, args.output)


if __name__ == "__main__":
    main()
