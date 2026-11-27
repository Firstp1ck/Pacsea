"""Resolve storyboard targets against the visible Ratatui cell capture."""

import argparse
from functools import cache
import json
from pathlib import Path
import re
from xml.etree import ElementTree

FOLDER = Path(__file__).resolve().parent
STORY = FOLDER / "storyboard.json"
CELL_W, CELL_H = 16, 29
BASELINE = 23
PAD_X, PAD_Y = 3, 1


@cache
def capture_rows(filename):
    root = ElementTree.parse(FOLDER / filename).getroot()
    rows = [[" "] * 116 for _ in range(34)]
    for run in root.findall(".//{*}text"):
        text = run.text or ""
        # A browser can collapse a gap and stretch the remaining glyphs to textLength.
        # Leave empty terminal cells between independently positioned text runs.
        if text.strip() != text or "  " in text:
            raise ValueError(f"Ambiguous SVG whitespace in {filename}: {text!r}")
        x, y = int(run.get("x")), int(run.get("y")) - BASELINE
        if x % CELL_W or y % CELL_H or int(run.get("textLength")) != len(text) * CELL_W:
            raise ValueError(f"Unaligned SVG run in {filename}: {text!r}")
        start = x // CELL_W
        rows[y // CELL_H][start:start + len(text)] = text
    return ["".join(row) for row in rows]


def target_rect(filename, anchor, origin):
    row, text = anchor
    line = capture_rows(filename)[row]
    if line.count(text) != 1:
        raise ValueError(f"{filename}, row {row}: expected one {text!r} in {line!r}")
    return [origin[0] + CELL_W * line.index(text) - PAD_X,
            origin[1] + CELL_H * row - PAD_Y,
            CELL_W * len(text) + 2 * PAD_X, CELL_H + 2 * PAD_Y]


def capture_changes(step, template):
    files = dict(re.findall(r'<img id="(frame-\d+)" class="terminal[^"]*" src="([^"]+)"', template))
    if not files:
        return [(0, re.search(r'<img class="terminal" src="([^"]+)"', template)[1])]
    changes = [(0, files["frame-1"])]
    for action in step.get("actions", []):
        if action.get("remove") == "hidden-frame":
            changes.append((action["at"], files[action["target"][1:]]))
    return changes


def check(story, write):
    html = (FOLDER / story["scenes"]).read_text()
    css = re.search(r"\.terminal \{[^}]*left: (\d+)px; top: (\d+)px; width: (\d+)px; height: (\d+)px", html)
    origin = tuple(map(int, css.groups()[:2]))
    if tuple(map(int, css.groups()[2:])) != (1856, 986):
        raise ValueError("Native terminal scale changed")
    templates = dict(re.findall(r'<template id="([^"]+)"[^>]*>(.*?)</template>', html, re.S))
    steps = {step["scene"]: step for step in story["steps"]}
    # Expected words, inputs and results from the walkthrough, independent of its cues.
    samples = [
        ("search-ripgrep", 5, 2, "ripgrep"),
        ("read-sources", 5, 4, "AUR"),
        ("read-markers", 1, 5, "[OOD]"), ("read-markers", 3, 5, "[ORPHAN]"),
        ("queue-choice", 3, 3, "extra ripgrep-all"), ("queue-choice", 5, 14, "extra ripgrep-all"),
        ("queue-history", 3, 15, "▶ fd"), ("queue-history", 6, 14, "extra fd"),
        ("export-list", 5, 31, "~/.config/pacsea/export/install_list_20250506_1.txt"),
        ("import-list", 3, 33, "[Shift+I]"), ("import-list", 5, 20, "vim"),
        ("import-list", 7, 21, "paru"),
        ("help-shortcuts", 1, 27, "[F1] Help"),
        ("read-size", 1, 5, "[DL: 2.1 MiB]"), ("read-size", 5.5, 5, "[Size: +6.5 MiB]"),
        ("keep-choice", 1, 4, "Preflight: Install"), ("keep-choice", 3, 14, "extra ripgrep-all"),
    ]
    count = 0
    for scene, step in steps.items():
        changes = capture_changes(step, templates[scene])
        for spot in step["spotlights"]:
            times = {spot["from"], *(t for t, _ in changes if spot["from"] < t < spot["to"])}
            for t in sorted(times):
                filename = [f for at, f in changes if at <= t][-1]
                rect = target_rect(filename, spot["anchor"], origin)
                if write and t == spot["from"]:
                    spot["rect"] = rect
                if spot["rect"] != rect:
                    raise ValueError(f"{scene} at {t}s: {filename} {spot['anchor']} needs {rect}")
                count += 1
    for scene, local, row, text in samples:
        step = steps[scene]
        spot = next(s for s in step["spotlights"] if s["from"] <= local < s["to"])
        filename = [f for t, f in capture_changes(step, templates[scene]) if t <= local][-1]
        if spot["rect"] != target_rect(filename, (row, text), origin):
            raise ValueError(f"{scene} at {local}s does not outline {text!r}")

    opening, closing = steps["import-list"]["actions"]
    moves = steps["import-list"]["caption"]["track"]
    if (len(moves) != 1 or abs(moves[0]["at"] - moves[0]["move"] - opening["at"]) > 1e-6
            or any(s["to"] > closing["at"] for s in steps["import-list"]["spotlights"])):
        raise ValueError("Import must hold until open, glide once, and release its outline on close")
    for scene in ("read-markers", "read-size", "reload-recipe"):
        if steps[scene]["caption"].get("track"):
            raise ValueError(f"{scene}: related targets share one unobstructed caption position")
    print(f"Verified {count} timed text targets and capture states.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="refresh renderer rectangles after editing the anchors")
    args = parser.parse_args()
    storyboard = json.loads(STORY.read_text())
    check(storyboard, args.write)
    if args.write:
        STORY.write_text(json.dumps(storyboard, ensure_ascii=False, indent=2) + "\n")
