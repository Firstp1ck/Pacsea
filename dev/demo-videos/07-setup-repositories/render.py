"""Render a Pacsea walkthrough with its Quiet Index arrangement and publication assets."""

import argparse
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw, ImageFont


def scene_times(story):
    starts = {}
    end = 0
    for step in story["steps"]:
        starts[step["scene"]] = end
        end += step["duration"]
    return starts, end


def synthesize(story, pilot, output):
    """Stretch whole two-bar cells between picture cues, preserving the pilot's synth and releases."""
    spec = importlib.util.spec_from_file_location("quiet_index", pilot / "audio/render.py")
    synth = importlib.util.module_from_spec(spec)
    # The pilot may live in another read-only checkout.
    sys.dont_write_bytecode = True
    spec.loader.exec_module(synth)
    starts, end = scene_times(story)
    phrases = story["music"]["phrases"]
    cue_times = [starts[phrase["scene"]] for phrase in phrases] + [end]
    arrangement = []
    boundaries = []
    for phrase, begin, finish in zip(phrases, cue_times[:-1], cue_times[1:], strict=True):
        cells = phrase["cells"]
        if finish <= begin or not cells:
            raise ValueError(f"Invalid music cue at {phrase['scene']}")
        boundaries.extend(round(synth.RATE * (begin + (finish - begin) * i / len(cells)))
                          for i in range(len(cells)))
        arrangement.extend(synth.SCORE[i] for i in cells)
    boundaries.append(round(end * synth.RATE))
    synth.SCORE = tuple(arrangement)
    command = ["ffmpeg", "-nostdin", "-v", "error", "-xerror", "-y",
               "-f", "f32le", "-ar", str(synth.RATE), "-ac", "2", "-i", "-",
               "-c:a", "aac", "-b:a", "192k", "-movie_timescale", str(synth.RATE),
               "-metadata", "title=" + story["music"]["title"], output]
    with subprocess.Popen(command, stdin=subprocess.PIPE) as encoder:
        with encoder.stdin as stream:
            for block in synth.blocks(boundaries):
                stream.write(block.tobytes())
        if encoder.wait():
            raise RuntimeError("FFmpeg could not encode Quiet Index")


def mux(story, picture, score, metadata, output):
    _, end = scene_times(story)
    chapters = story["chapters"]
    entries = [";FFMETADATA1"]
    for i, chapter in enumerate(chapters):
        stop = chapters[i + 1]["start"] if i + 1 < len(chapters) else end
        title = chapter["title"].replace("\\", "\\\\")
        for character in "=;#\n":
            title = title.replace(character, "\\" + character)
        entries.extend(["[CHAPTER]", "TIMEBASE=1/1000", f"START={round(chapter['start'] * 1000)}",
                        f"END={round(stop * 1000)}", "title=" + title])
    metadata.write_text("\n".join(entries) + "\n")
    subprocess.run(["ffmpeg", "-nostdin", "-v", "error", "-xerror", "-y",
                    "-i", picture, "-i", score, "-i", metadata, "-map", "0:v:0", "-map", "1:a:0",
                    "-map_metadata", "2", "-map_chapters", "2", "-metadata", "title=" + story["title"],
                    "-metadata:s:a:0", "title=" + story["music"]["title"],
                    "-c", "copy", "-movflags", "+faststart", output], check=True)


def verify_delivery(folder, story, movie):
    """Catch stale silent masters and keep the description in sync with embedded chapters."""
    probe = json.loads(subprocess.check_output([
        "ffprobe", "-v", "error", "-show_streams", "-show_chapters", "-of", "json", movie,
    ]))
    _, duration = scene_times(story)
    streams = {stream["codec_type"]: stream for stream in probe["streams"]}
    video, audio = streams["video"], streams["audio"]
    picture = (video["codec_name"], video["width"], video["height"], video["avg_frame_rate"],
               int(video["nb_frames"]), float(video["duration"]))
    if picture != ("h264", story["width"], story["height"], f"{story['fps']}/1",
                   round(duration * story["fps"]), duration):
        raise ValueError("The picture does not match the storyboard dimensions, frame count or timing")
    if (audio["codec_name"], audio["channels"], audio["sample_rate"], float(audio["duration"])) != (
        "aac", 2, "48000", duration,
    ):
        raise ValueError("The score must span the entire picture in 48 kHz stereo AAC")
    description = (folder / "youtube-description.txt").read_text().splitlines()
    for expected, actual in zip(story["chapters"], probe["chapters"], strict=True):
        start, title = expected["start"], expected["title"]
        stamp = f"{int(start // 60):02}:{int(start % 60):02} {title}"
        if (float(actual["start_time"]), actual["tags"]["title"]) != (start, title) or stamp not in description:
            raise ValueError(f"Check the MP4 chapter and description entry for {stamp}")


def contact_sheet(story, movie, output):
    steps = story["steps"]
    cols = 4
    sheet = Image.new("RGB", (cols * 492, ((len(steps) + cols - 1) // cols) * 313), "#10121b")
    draw = ImageDraw.Draw(sheet)
    face = ImageFont.truetype("DejaVuSans.ttf", 17)
    start = 0
    for i, step in enumerate(steps):
        offset = step["duration"] - (3 if step["scene"].endswith("-title") else 1)
        still = subprocess.check_output(["ffmpeg", "-nostdin", "-v", "error", "-xerror",
                                        "-ss", str(start + offset), "-i", movie, "-map", "0:v:0",
                                        "-frames:v", "1", "-vf", "scale=480:270:flags=lanczos",
                                        "-c:v", "png", "-f", "image2pipe", "-"])
        x, y = i % cols * 492 + 6, i // cols * 313 + 6
        with Image.open(io.BytesIO(still)) as frame:
            sheet.paste(frame, (x, y))
        headings = {"open-title": "Opening", "chapter-title": "Chapter transition", "close-title": "Outcome and credits"}
        label = headings.get(step["scene"], step["caption"]["bold"].rstrip(".,"))
        draw.text((x + 2, y + 277), f"{int(start // 60):02}:{int(start % 60):02}    {label}", font=face, fill="#e3eafa")
        start += step["duration"]
    sheet.save(output, optimize=True)


def render_episode(folder):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("skill", type=Path, help="installed explainer-video directory")
    parser.add_argument("--pilot", type=Path, default=folder.parent / "01-discovery-pilot")
    parser.add_argument("--visual", type=Path, help="reuse an existing silent master of this storyboard")
    args = parser.parse_args()
    story = json.loads((folder / "storyboard.json").read_text())
    with tempfile.TemporaryDirectory() as scratch, tempfile.TemporaryDirectory(dir=folder) as staging:
        temp, stage = Path(scratch), Path(staging)
        visual = args.visual or temp / "picture.mp4"
        if not args.visual:
            subprocess.run(["node", args.skill / "scripts/render.mjs", folder / "storyboard.json",
                            "--crf", "17", "--out", visual], check=True)
        score, movie = stage / "quiet-index.m4a", stage / "video.mp4"
        synthesize(story, args.pilot, score)
        mux(story, visual, score, temp / "chapters.ffmeta", movie)
        verify_delivery(folder, story, movie)
        contact_sheet(story, movie, stage / "sheet.png")
        subprocess.run(["inkscape", folder / "thumbnail.svg", "-o", stage / "thumbnail.png"], check=True)
        (folder / "audio").mkdir(exist_ok=True)
        score.replace(folder / "audio" / score.name)
        for name in ("thumbnail.png", "sheet.png", "video.mp4"):
            (stage / name).replace(folder / name)


if __name__ == "__main__":
    render_episode(Path(__file__).resolve().parent)
