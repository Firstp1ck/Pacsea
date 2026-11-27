"""Rebuild sheet.png from representative frames in the finished MP4."""

import io
import json
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


# Times are local to each shot. Pick the result of the action after it settles.
SHOTS = [
    ("discovery-intro", 2, "Package discovery"),
    ("search-ripgrep", 6, "Search by package name"),
    ("read-sources", 5.8, "Read the sources"),
    ("clear-and-toggle", 5.7, "Enable fuzzy matching"),
    ("match-letters", 5, "Match letters in order"),
    ("recall-search", 5.8, "Restore a search"),
    ("browse-matches", 4.95, "Browse results"),
    ("read-markers", 5, "Read the flags"),
    ("filter-sources", 2.65, "Filter by source"),
    ("sort-packages", 2.9, "Sort by popularity"),
    ("community-comments", 4.5, "Read comments"),
    ("vote-dialog", 4.3, "Preview a vote"),
    ("inspect-aur-recipe", 9.75, "Open the AUR recipe"),
    ("reload-recipe", 5.85, "Reload the recipe"),
    ("run-checks", 5.1, "Run static PKGBUILD checks"),
    ("read-checks", 4.7, "Read the results"),
    ("build-install-list", 1.5, "Build the install list"),
    ("queue-choice", 6.8, "Queue an official result"),
    ("queue-history", 3.9, "Queue from history"),
    ("prune-list", 6.9, "Find and remove a queued row"),
    ("export-list", 5.75, "Export the list"),
    ("import-list", 6.3, "Read the import format"),
    ("help-shortcuts", 3, "Look up shortcuts"),
    ("review-list", 5.5, "Open Preflight"),
    ("read-size", 5.5, "Compare sizes"),
    ("keep-choice", 7, "Keep the reviewed package queued"),
    ("end-credits", 3, "Credits"),
]


def main():
    folder = Path(__file__).resolve().parent
    story = json.loads((folder / "storyboard.json").read_text())
    rows = (len(SHOTS) + 4) // 5
    sheet = Image.new("RGB", (3260, 10 + rows * 406), "#10121b")
    draw = ImageDraw.Draw(sheet)
    time_font = ImageFont.truetype("DejaVuSansMono.ttf", 18)
    label_font = ImageFont.truetype("DejaVuSans.ttf", 21)
    start = 0
    for i, ((scene, offset, label), step) in enumerate(zip(SHOTS, story["steps"], strict=True)):
        if scene != step["scene"] or not 0 <= offset < step["duration"]:
            raise ValueError(f"Update the contact sheet selection for {step['scene']}")
        data = subprocess.check_output([
            "ffmpeg", "-nostdin", "-v", "error", "-xerror", "-ss", str(start + offset),
            "-i", folder / "video.mp4", "-map", "0:v:0", "-frames:v", "1",
            "-vf", "scale=640:360:flags=lanczos", "-c:v", "png", "-f", "image2pipe", "-",
        ])
        x, y = 10 + i % 5 * 650, 10 + i // 5 * 406
        with Image.open(io.BytesIO(data)) as frame:
            sheet.paste(frame, (x, y))
        minutes, seconds = divmod(start, 60)
        stamp = f"{int(minutes)}:{seconds:05.2f}".rstrip("0").rstrip(".")
        draw.text((x + 3, y + 367), stamp, font=time_font, fill="#8cdfff")
        draw.text((x + 92, y + 365), label, font=label_font, fill="#edf1f6")
        start += step["duration"]
    sheet.save(folder / "sheet.png", optimize=True)


if __name__ == "__main__":
    main()
