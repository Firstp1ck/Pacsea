"""Render episode 08 with the same picture, score and chapter pipeline as episode 07."""

from pathlib import Path
import runpy


if __name__ == "__main__":
    folder = Path(__file__).resolve().parent
    pipeline = runpy.run_path(str(folder.parent / "07-setup-repositories" / "render.py"))
    pipeline["render_episode"](folder)
