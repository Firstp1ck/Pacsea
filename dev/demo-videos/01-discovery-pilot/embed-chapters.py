"""Combine the pilot's video, original score and chapter markers without re-encoding."""

import argparse
import json
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("master", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    folder = Path(__file__).resolve().parent
    story = json.loads((folder / "storyboard.json").read_text())
    duration = sum(step["duration"] for step in story["steps"])
    chapters = story["chapters"]
    metadata = [";FFMETADATA1", f"title={story['title']}", "artist=Pacsea"]
    for chapter, end in zip(chapters, [c["start"] for c in chapters[1:]] + [duration], strict=True):
        title = chapter["title"]
        for char in ("\\", "=", ";", "#", "\n"):
            title = title.replace(char, "\\" + char)
        metadata.extend(("[CHAPTER]", "TIMEBASE=1/1000", f"START={round(chapter['start'] * 1000)}",
                         f"END={round(end * 1000)}", f"title={title}"))
    subprocess.run([
        "ffmpeg", "-v", "error", "-xerror", "-y", "-i", args.master,
        "-f", "ffmetadata", "-i", "pipe:0", "-i", folder / "audio/quiet-index.m4a",
        "-map", "0:v:0", "-map", "2:a:0", "-c", "copy",
        "-metadata:s:a:0", "title=Quiet Index", "-disposition:a:0", "default",
        "-map_metadata", "1", "-map_chapters", "1", "-movflags", "+faststart", args.output,
    ], input="\n".join(metadata) + "\n", text=True, check=True)


if __name__ == "__main__":
    main()
