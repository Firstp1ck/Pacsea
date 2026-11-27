"""Build one episode from its storyboard. Run from the integrated series checkout."""

import argparse
import runpy
import io
import json
import subprocess
import tempfile
from pathlib import Path
from types import SimpleNamespace

import numpy as np
from PIL import Image, ImageDraw, ImageFont


def run(*args):
    subprocess.run(args, check=True)


def render_score(story, pilot, path):
    """Arrange the Quiet Index instruments around each gesture and reading hold."""
    instrument = SimpleNamespace(**runpy.run_path(str(pilot / "audio" / "render.py")))
    chords = list(instrument.SCORE)
    # Give each episode its own entrance into the recurring harmonic sequence.
    first_chord = {"04": 0, "05": 6, "06": 12}[path.parent.name[:2]]
    overlap = round(1.25 * instrument.RATE)
    carry = np.zeros((overlap, 2))
    command = ["ffmpeg", "-nostdin", "-v", "error", "-y", "-f", "f32le",
               "-ar", str(instrument.RATE), "-ac", "2", "-i", "pipe:0",
               "-c:a", "aac", "-b:a", "192k", "-movie_timescale", str(instrument.RATE), path]
    with subprocess.Popen(command, stdin=subprocess.PIPE) as encoder:
        with encoder.stdin as stream:
            for index, shot in enumerate(story["steps"]):
                length = shot["duration"]
                count = round(length * instrument.RATE)
                t = np.arange(count + overlap) / instrument.RATE
                chord, notes, call, _ = chords[(first_chord + index) % len(chords)]
                strokes = (call, instrument.REST)
                if shot["scene"] == "opening":
                    chord, notes, strokes = "D9", (), (instrument.REST, instrument.ANSWER)
                elif shot["scene"] == "credits":
                    chord, notes, strokes = "D6", ((1, 62),), (instrument.DOWN, instrument.REST)
                elif shot["scene"].startswith("chapter-"):
                    chord, notes, strokes = "Bm7", ((3, 66),), (instrument.DOWN, instrument.REST)
                signal = instrument.chord(t, length, overlap / instrument.RATE, chord, index)
                instrument.add_figure(signal, length, notes, index)
                instrument.add_pulse(signal, length, chord, strokes)
                # Put a single muted pluck on the nearest eighth to the first UI change.
                changes = [a["at"] for a in shot.get("actions", []) if a["type"] in ("text", "type")]
                if changes:
                    beat = min(15, max(1, round(min(changes) * 16 / length)))
                    accent = ((beat % 8, 1, 0.53),)
                    instrument.add_pulse(signal, length, chord, (accent, ()) if beat < 8 else ((), accent))
                signal[:overlap] += carry
                carry = signal[count:]
                output = signal[:count]
                if shot["scene"] == "credits":
                    output *= instrument.rise((length - 0.06 - t[:count]) / length)[:, None]
                stream.write((output * 0.19).astype("<f4").tobytes())
        if encoder.wait():
            raise RuntimeError("The Quiet Index AAC encode failed")


def make_sheet(story, master, destination):
    width, height, columns = 480, 270, 4
    steps = story["steps"]
    sheet = Image.new("RGB", (columns * (width + 8) + 8, ((len(steps) + columns - 1) // columns) * (height + 40) + 8), "#131622")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.truetype("DejaVuSans.ttf", 18)
    stamp_font = ImageFont.truetype("DejaVuSansMono.ttf", 17)
    start = 0
    for i, shot in enumerate(steps):
        offset = 3 if not shot["caption"]["number"] else shot["duration"] - 0.65
        frame = subprocess.check_output([
            "ffmpeg", "-nostdin", "-v", "error", "-ss", str(start + offset), "-i", master,
            "-frames:v", "1", "-vf", f"scale={width}:{height}:flags=lanczos", "-c:v", "png", "-f", "image2pipe", "-"])
        x, y = 8 + i % columns * (width + 8), 8 + i // columns * (height + 40)
        with Image.open(io.BytesIO(frame)) as img:
            sheet.paste(img, (x, y))
        draw.text((x, y + height + 6), f"{start // 60:02}:{start % 60:02}", font=stamp_font, fill="#8bdcf3")
        draw.text((x + 65, y + height + 5), shot["caption"]["bold"], font=font, fill="#e6e9f2")
        start += shot["duration"]
    sheet.save(destination)


def assemble(folder, pilot, skill, picture):
    story = json.loads((folder / "storyboard.json").read_text())
    duration = sum(step["duration"] for step in story["steps"])
    with tempfile.TemporaryDirectory() as temp:
        work = Path(temp) / folder.name
        work.mkdir()
        if picture is None:
            picture = work / "picture.mp4"
            run("node", skill / "scripts/render.mjs", folder / "storyboard.json", "--crf", "17", "--out", picture)
        score = work / "quiet-index.m4a"
        render_score(story, pilot, score)
        lines = [";FFMETADATA1"]
        for i, c in enumerate(story["chapters"]):
            end = story["chapters"][i + 1]["start"] if i + 1 < len(story["chapters"]) else duration
            lines.extend(["[CHAPTER]", "TIMEBASE=1/1000", f"START={c['start'] * 1000}", f"END={end * 1000}", f"title={c['title']}"])
        metadata = work / "chapters.txt"
        metadata.write_text("\n".join(lines) + "\n")
        with tempfile.TemporaryDirectory(dir=folder) as stage:
            master = Path(stage) / "video.mp4"
            run("ffmpeg", "-nostdin", "-v", "error", "-xerror", "-y", "-i", picture, "-i", score,
                "-i", metadata, "-map", "0:v:0", "-map", "1:a:0", "-map_chapters", "2", "-c", "copy",
                "-metadata", f"title={story['title']}", "-metadata", "artist=Pacsea / Firstpick",
                "-metadata", "comment=Original music: Quiet Index. Illustrative session.", "-movflags", "+faststart", master)
            make_sheet(story, master, Path(stage) / "sheet.png")
            run("inkscape", folder / "thumbnail.svg", "-o", Path(stage) / "thumbnail.png")
            for name in ("video.mp4", "sheet.png", "thumbnail.png"):
                (Path(stage) / name).replace(folder / name)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--pilot", type=Path, help="discovery pilot folder, if outside the integrated checkout")
    parser.add_argument("--skill", type=Path, default=Path.home() / ".codex/skills/explainer-video")
    parser.add_argument("--picture", type=Path, help="existing visual master from this storyboard")
    args = parser.parse_args()
    folder = args.folder.resolve()
    assemble(folder, args.pilot or folder.parent / "01-discovery-pilot", args.skill, args.picture)
