#!/usr/bin/env python3
"""Crop the macOS window shadow and normalize a window capture to logical pixels.

screencapture -l includes a 23-logical-pixel shadow on every side. On Retina
the captured PNG is 2x. The opaque part of the PNG is the window content; use
its alpha=255 bounding box, then resize to the CGWindow logical bounds.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input")
    parser.add_argument("output")
    parser.add_argument("--width", type=int, required=True)
    parser.add_argument("--height", type=int, required=True)
    parser.add_argument("--metadata")
    args = parser.parse_args()

    image = Image.open(args.input).convert("RGBA")
    alpha = image.getchannel("A")
    opaque = alpha.point(lambda value: 255 if value == 255 else 0)
    box = opaque.getbbox() or image.getbbox()
    if box is None:
        raise SystemExit(f"{args.input}: no visible window pixels")
    content = image.crop(box)
    scale = content.width / args.width if args.width else 1.0
    logical = content.resize((args.width, args.height), Image.Resampling.LANCZOS)
    logical.save(args.output)
    metadata = {
        "source": str(Path(args.input).name),
        "source_pixels": image.size,
        "opaque_bbox": box,
        "content_pixels": content.size,
        "scale": round(scale, 4),
        "logical_size": [args.width, args.height],
    }
    if args.metadata:
        Path(args.metadata).write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
