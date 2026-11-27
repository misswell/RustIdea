#!/usr/bin/env python3
"""Compare two normalized app screenshots and write evidence.

The output directory receives openidea.png, rustidea.png, diff.png and
metrics.json. Masks are logical-pixel [x, y, w, h] rectangles recorded in the
scenario manifest; they are filled with black in both images and are counted
in metrics.json so a large mask cannot silently inflate the score.
"""
from __future__ import annotations

import argparse
import json
import math
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def run(cmd: list[str]) -> str:
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def ssim(expected: Path, actual: Path) -> float | None:
    try:
        output = subprocess.run(
            ["ffmpeg", "-hide_banner", "-nostdin", "-i", str(expected), "-i", str(actual),
             "-lavfi", "ssim", "-f", "null", "-"],
            check=True, capture_output=True, text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return None
    for token in output.stderr.replace("|", " ").split():
        if token.startswith("All:"):
            try:
                return float(token[4:])
            except ValueError:
                return None
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("expected")
    parser.add_argument("actual")
    parser.add_argument("out_dir")
    parser.add_argument("--masks", default="[]", help="JSON list of [x,y,w,h] logical rects")
    parser.add_argument("--threshold", type=float, default=0.98)
    parser.add_argument("--report-only", action="store_true")
    args = parser.parse_args()

    expected = Image.open(args.expected).convert("RGB")
    actual = Image.open(args.actual).convert("RGB")
    if expected.size != actual.size:
        raise SystemExit(
            f"geometry mismatch: expected {expected.size}, actual {actual.size}. "
            "Capture both apps at the same window size before comparing."
        )
    masks = json.loads(args.masks)
    for mask in masks:
        if len(mask) != 4:
            raise SystemExit(f"bad mask {mask!r}")
        x, y, w, h = (int(v) for v in mask)
        if x < 0 or y < 0 or w < 0 or h < 0 or x + w > expected.width or y + h > expected.height:
            raise SystemExit(f"mask {mask!r} outside {expected.size}")
        for image in (expected, actual):
            image.paste((0, 0, 0), (x, y, x + w, y + h))

    diff = ImageChops.difference(expected, actual)
    stat = ImageStat.Stat(diff)
    mae = sum(stat.mean) / len(stat.mean)
    rmse = math.sqrt(sum(v * v for v in stat.rms) / len(stat.rms))
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    expected.save(out / "openidea.png")
    actual.save(out / "rustidea.png")
    amplified = diff.point(lambda value: min(255, value * 4))
    amplified.save(out / "diff.png")
    with tempfile.TemporaryDirectory() as td:
        e = Path(td) / "e.png"
        a = Path(td) / "a.png"
        expected.save(e)
        actual.save(a)
        score = ssim(e, a)
    metrics = {
        "expected": str(args.expected),
        "actual": str(args.actual),
        "size": list(expected.size),
        "masks": masks,
        "masked_pixels": sum(w * h for _, _, w, h in masks),
        "masked_ratio": sum(w * h for _, _, w, h in masks) / (expected.width * expected.height),
        "mae": round(mae, 4),
        "rmse": round(rmse, 4),
        "ssim": None if score is None else round(score, 6),
        "threshold": args.threshold,
        "pass": bool(score is not None and score >= args.threshold),
    }
    (out / "metrics.json").write_text(json.dumps(metrics, indent=2) + "\n")
    print(json.dumps(metrics, indent=2))
    if args.report_only:
        return 0
    return 0 if metrics["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
