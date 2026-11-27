"""Render the discovery episode and extract its approval copies from the muxed master."""

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path


FOLDER = Path(__file__).resolve().parent
EXCERPTS = {
    "approval-excerpts.mp4": [(0, 15), (120, 138.25), (194, 211)],
    "target-alignment-excerpts.mp4": [(7.25, 16.75), (162.75, 175.5)],
}


def excerpt(master, output, spans):
    command = ["ffmpeg", "-nostdin", "-v", "error", "-xerror", "-y"]
    for start, end in spans:
        command.extend(["-ss", str(start), "-t", str(end - start), "-i", master])
    inputs = "".join(f"[{i}:v:0][{i}:a:0]" for i in range(len(spans)))
    command.extend(["-filter_complex", f"{inputs}concat=n={len(spans)}:v=1:a=1[v][a]",
                    "-map", "[v]", "-map", "[a]", "-map_chapters", "-1", "-map_metadata", "-1",
                    "-c:v", "libx264", "-crf", "17", "-c:a", "aac", "-b:a", "192k",
                    "-movflags", "+faststart", output])
    subprocess.run(command, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("skill", type=Path, help="explainer-video skill directory")
    args = parser.parse_args()
    renderer = args.skill.resolve() / "scripts/render.mjs"
    subprocess.run([sys.executable, FOLDER / "check-targets.py"], check=True)
    # Keep the large picture master in build scratch. Stage the finished files beside
    # their destinations so publication also works when scratch is on another disk.
    with tempfile.TemporaryDirectory() as temp, tempfile.TemporaryDirectory(dir=FOLDER) as stage:
        master = Path(temp) / "picture.mp4"
        output = Path(stage) / "episode.mp4"
        subprocess.run(["node", renderer, FOLDER / "storyboard.json",
                        "--crf", "17", "--out", master], check=True)
        subprocess.run([sys.executable, FOLDER / "audio/render.py"], check=True)
        subprocess.run([sys.executable, FOLDER / "embed-chapters.py", master, output], check=True)
        for name, spans in EXCERPTS.items():
            excerpt(output, Path(stage) / name, spans)
        for name in EXCERPTS:
            (Path(stage) / name).replace(FOLDER / name)
        output.replace(FOLDER / "video.mp4")
        subprocess.run([sys.executable, FOLDER / "contact-sheet.py"], check=True)


if __name__ == "__main__":
    main()
