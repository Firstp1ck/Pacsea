"""Render the episode with the explainer-video skill, the Quiet Index score and embedded chapters."""

import argparse
import io
import json
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw, ImageFont

FOLDER = Path(__file__).resolve().parent
PILOT = FOLDER.with_name("01-discovery-pilot")


def check_targets(story):
    """Resolve the visible SVG at every change and check both ends of each outlined label."""
    geometry = runpy.run_path(str(PILOT / "check-targets.py"))
    html = (FOLDER / story["scenes"]).read_text()
    templates = dict(re.findall(r'<template id="([^"]+)"[^>]*>(.*?)</template>', html, re.S))
    for source in re.findall(r'<img [^>]*src="([^"]+)"', html):
        if Path(source).is_absolute() or not (FOLDER / source).is_file():
            raise ValueError(f"Missing or nonrelative scene asset: {source}")
    for step in story["steps"]:
        changes = geometry["capture_changes"](step, templates[step["scene"]])
        for spot in step["spotlights"]:
            times = [spot["from"], *(t for t, _ in changes if spot["from"] < t < spot["to"])]
            for time in times:
                source = next(name for t, name in reversed(changes) if t <= time)
                rect = geometry["target_rect"](str(FOLDER / source), spot["anchor"], (32, 64))
                if rect != spot["rect"]:
                    raise ValueError(f"{step['scene']} at {time}s: {spot['anchor']} needs {rect}")


def score_boundaries(story, cells, rate):
    """Assign whole phrases to the marked picture cues and interpolate the intervening bars."""
    steps = {}
    duration = 0
    for step in story["steps"]:
        steps[step["scene"]] = duration, step
        duration += step["duration"]
    anchors = []
    for cue in story["scoreCues"]:
        start, step = steps[cue["scene"]]
        if "target" in cue:
            offsets = [a["at"] for a in step["actions"] if a["target"] == cue["target"]]
            if len(offsets) != 1:
                raise ValueError(f"Expected one musical cue in {cue['scene']}: {cue['target']}")
            start += offsets[0]
        anchors.append((cue["cell"], start))
    anchors.append((cells, duration))
    if anchors[0] != (0, 0):
        raise ValueError("The score must begin with the film")
    boundaries = []
    for (first, start), (last, end) in zip(anchors, anchors[1:]):
        if last <= first or end <= start:
            raise ValueError("Score cells and picture cues must advance together")
        for cell in range(first, last):
            boundaries.append(round(rate * (start + (end - start) * (cell - first) / (last - first))))
    return [*boundaries, round(rate * duration)]


def render_score(story, output):
    """Play the pilot's original instruments and notes at the canonical episode's cue times."""
    score = runpy.run_path(str(PILOT / "audio/render.py"))
    rate = score["RATE"]
    boundaries = score_boundaries(story, len(score["SCORE"]), rate)
    command = [
        "ffmpeg", "-nostdin", "-v", "error", "-y", "-f", "f32le", "-ar", str(rate),
        "-ac", "2", "-i", "pipe:0", "-c:a", "aac", "-b:a", "192k",
        "-movie_timescale", str(rate), "-metadata", f"title={score['TITLE']}", output,
    ]
    with subprocess.Popen(command, stdin=subprocess.PIPE) as encoder:
        with encoder.stdin as stream:
            for block in score["blocks"](boundaries):
                stream.write(block.tobytes())
        if encoder.wait():
            raise RuntimeError("Could not encode Quiet Index")


def contact_sheet(story, movie, output):
    """Show one selected frame per shot, labeled with its start in the final movie."""
    steps = story["steps"]
    columns = 4
    rows = (len(steps) + columns - 1) // columns
    sheet = Image.new("RGB", (columns * 490 + 10, rows * 307 + 10), "#10121b")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.truetype("DejaVuSans.ttf", 17)
    time_font = ImageFont.truetype("DejaVuSansMono.ttf", 15)
    start = 0
    for i, step in enumerate(steps):
        if not 0 <= step["sheetAt"] < step["duration"]:
            raise ValueError(f"Choose a contact sheet frame inside {step['scene']}")
        frame = subprocess.check_output([
            "ffmpeg", "-nostdin", "-v", "error", "-xerror", "-ss", str(start + step["sheetAt"]),
            "-i", movie, "-map", "0:v:0", "-frames:v", "1", "-vf", "scale=480:270:flags=lanczos",
            "-c:v", "png", "-f", "image2pipe", "-",
        ])
        x, y = 10 + i % columns * 490, 10 + i // columns * 307
        with Image.open(io.BytesIO(frame)) as image:
            sheet.paste(image, (x, y))
        m, s = divmod(int(start), 60)
        draw.text((x, y + 276), f"{m}:{s:02d}", font=time_font, fill="#89dceb")
        draw.text((x + 58, y + 274), step["caption"]["bold"], font=font, fill="#eef1f7")
        start += step["duration"]
    sheet.save(output, optimize=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("skill", type=Path, help="explainer-video skill directory")
    parser.add_argument("--picture", type=Path, help="use a previously rendered silent picture master")
    args = parser.parse_args()
    story_path = FOLDER / "storyboard.json"
    story = json.loads(story_path.read_text())
    check_targets(story)
    with tempfile.TemporaryDirectory() as scratch, tempfile.TemporaryDirectory(dir=FOLDER) as staging:
        work, stage = Path(scratch), Path(staging)
        movie = stage / "video.mp4"
        picture = args.picture or work / "picture.mp4"
        if args.picture is None:
            subprocess.run([
                "node", args.skill.resolve() / "scripts/render.mjs", story_path,
                "--crf", "17", "--out", picture,
            ], check=True)
        # The shared muxer reads its score and storyboard beside the script. Stage them
        # together so a different edit can never modify the approved pilot's soundtrack.
        (work / "audio").mkdir()
        render_score(story, work / "audio/quiet-index.m4a")
        shutil.copyfile(story_path, work / "storyboard.json")
        muxer = work / "embed-chapters.py"
        shutil.copyfile(PILOT / muxer.name, muxer)
        subprocess.run([sys.executable, muxer, picture, movie], check=True)
        contact_sheet(story, movie, stage / "sheet.png")
        movie.replace(FOLDER / movie.name)
        (stage / "sheet.png").replace(FOLDER / "sheet.png")


if __name__ == "__main__":
    main()
