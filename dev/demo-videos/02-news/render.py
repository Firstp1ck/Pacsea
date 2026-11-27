"""Render an episode from its stock explainer-video storyboard and the pilot's score."""

import argparse
import json
import math
from pathlib import Path
import runpy
import subprocess
import tempfile

import numpy as np
from PIL import Image, ImageDraw, ImageFont


def run(command):
    subprocess.run(command, check=True)


def changing_frames(story):
    """Sample the stock player at 24 fps through motion. Reuse a still through each hold."""
    fps = story["fps"]
    frames = set()
    offset = 0

    def include(start, end):
        first = math.floor((offset + start) * fps)
        last = math.ceil((offset + end) * fps)
        frames.update(range(first, last + 1))

    for step in story["steps"]:
        include(0, 0)
        for action in step.get("actions", []):
            start = action["at"]
            if action["type"] == "type":
                for index in range(len(action["text"]) + 1):
                    # Capture the first displayed frame of each typed glyph.
                    frames.add(math.ceil((offset + start + index / action["cps"]) * fps))
            else:
                include(start, start + action.get("dur", 0))
        for key in step["caption"].get("track", []):
            include(key["at"] - key["move"], key["at"])
        for spot in step.get("spotlights", []):
            include(spot["from"], spot["from"])
            if "to" in spot:
                include(spot["to"], spot["to"])
        offset += step["duration"]
    count = round(offset * fps)
    return sorted(frame for frame in frames if 0 <= frame < count), count


def picture(story, folder, skill, scratch):
    frames, count = changing_frames(story)
    fps = story["fps"]
    # A sub-microsecond offset avoids rounding a calculated glyph boundary backwards.
    times = [frame / fps + 0.0000001 for frame in frames]
    run(["node", skill / "scripts/render.mjs", folder / "storyboard.json", "--no-video",
         "--stills", ",".join(f"{t:.9f}" for t in times), "--stills-dir", scratch])
    lines = ["ffconcat version 1.0"]
    for index, (frame, t) in enumerate(zip(frames, times)):
        next_frame = frames[index + 1] if index + 1 < len(frames) else count
        lines.extend([f"file 'storyboard-{t:.2f}s.png'", f"option framerate {fps}",
                      f"duration {(next_frame - frame) / fps:.9f}"])
    # FFmpeg needs an end packet to honor the last still's duration.
    lines.extend([f"file 'storyboard-{times[-1]:.2f}s.png'", f"option framerate {fps}"])
    manifest = scratch / "picture.ffconcat"
    manifest.write_text("\n".join(lines) + "\n")
    visual = scratch / "picture.mp4"
    run(["ffmpeg", "-v", "error", "-nostdin", "-f", "concat", "-safe", "0", "-i", manifest,
         "-vf", f"fps={fps}", "-frames:v", str(count), "-c:v", "libx264", "-threads", "2",
         "-preset", "veryfast", "-crf", "17", "-pix_fmt", "yuv420p", visual])
    return visual


def soundtrack(story, pilot, starts, duration, scratch):
    score = runpy.run_path(str(pilot / "audio/render.py"))
    anchors = [(0, 0)]
    anchors.extend((int(cell), starts[scene])
                   for cell, scene in story["production"]["scoreAnchors"].items())
    anchors.append((len(score["SCORE"]), duration))
    cells, times = zip(*sorted(anchors))
    boundaries = np.rint(np.interp(np.arange(cells[-1] + 1), cells, times) * score["RATE"])
    audio = scratch / "music.m4a"
    command = ["ffmpeg", "-v", "error", "-nostdin", "-f", "f32le", "-ar", str(score["RATE"]),
               "-ac", "2", "-i", "pipe:0", "-c:a", "aac", "-b:a", "192k", audio]
    with subprocess.Popen(command, stdin=subprocess.PIPE) as process:
        with process.stdin as stream:
            for block in score["blocks"](boundaries.astype(int)):
                stream.write(block.tobytes())
        if process.wait():
            raise RuntimeError("Quiet Index encoding failed")
    return audio


def contact_sheet(story, starts, video, output, scratch):
    width, height = 400, 225
    count = len(story["steps"])
    sheet = Image.new("RGB", (width * 4, (height + 32) * math.ceil(count / 4)), "#10131c")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.truetype("DejaVuSans.ttf", 16)
    for index, step in enumerate(story["steps"]):
        start = starts[step["scene"]]
        t = start + step["duration"] * .68
        still = scratch / "sheet-frame.png"
        run(["ffmpeg", "-v", "error", "-nostdin", "-y", "-ss", str(t), "-i", video,
             "-frames:v", "1", "-vf", f"scale={width}:{height}:flags=lanczos", still])
        x, y = index % 4 * width, index // 4 * (height + 32)
        with Image.open(still) as image:
            sheet.paste(image, (x, y))
        label = f"{int(start // 60):02}:{int(start % 60):02}   {step['caption']['bold']}"
        draw.text((x + 9, y + height + 5), label, fill="#dce3f0", font=font)
    sheet.save(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("episode", type=Path)
    parser.add_argument("skill", type=Path)
    parser.add_argument("--pilot", type=Path, help="pilot source when working in a partial checkout")
    args = parser.parse_args()
    folder = args.episode.resolve()
    pilot = args.pilot or folder.parent / "01-discovery-pilot"
    story = json.loads((folder / "storyboard.json").read_text())
    starts, duration = {}, 0
    for step in story["steps"]:
        starts[step["scene"]] = duration
        duration += step["duration"]
    with tempfile.TemporaryDirectory(prefix="pacsea-edition-") as temp:
        scratch = Path(temp)
        visual = picture(story, folder, args.skill.resolve(), scratch)
        audio = soundtrack(story, pilot, starts, duration, scratch)
        chapters = story["chapters"]
        meta = [";FFMETADATA1", "title=" + story["title"]]
        for index, chapter in enumerate(chapters):
            end = chapters[index + 1]["start"] if index + 1 < len(chapters) else duration
            meta.extend(["[CHAPTER]", "TIMEBASE=1/1000", f"START={round(chapter['start'] * 1000)}",
                         f"END={round(end * 1000)}", "title=" + chapter["title"]])
        metadata = scratch / "chapters.txt"
        metadata.write_text("\n".join(meta) + "\n")
        with tempfile.TemporaryDirectory(dir=folder) as stage:
            video = Path(stage) / "video.mp4"
            run(["ffmpeg", "-v", "error", "-nostdin", "-i", visual, "-i", audio,
                 "-f", "ffmetadata", "-i", metadata, "-map", "0:v", "-map", "1:a",
                 "-map_metadata", "2", "-map_chapters", "2", "-c", "copy",
                 "-movflags", "+faststart", video])
            contact_sheet(story, starts, video, Path(stage) / "sheet.png", scratch)
            run(["inkscape", folder / "thumbnail.svg", "-o", Path(stage) / "thumbnail.png"])
            for name in ("video.mp4", "sheet.png", "thumbnail.png"):
                (Path(stage) / name).replace(folder / name)


if __name__ == "__main__":
    main()
