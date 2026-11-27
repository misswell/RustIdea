#!/usr/bin/env python3
"""Validate the OpenIDEA visual baseline contract.

The manifest is a spec, so it must not drift from the files:

- ids are unique and numbered 01..NN
- captured scenarios point at a real PNG whose pixels match the declared geometry
- no two captured scenarios share the same image (a copy is not a capture)
- pending scenarios do not ship an image that could be mistaken for a baseline
- masks are well formed rectangles in image coordinates

Run from the repository root or CI; exits non-zero on the first violated rule.
"""

from __future__ import annotations

import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINES = ROOT / "tests" / "visual-baselines"
MANIFEST = BASELINES / "manifest.json"
ENVIRONMENT = BASELINES / "environment.json"
STATUSES = {"captured-reference", "pending-capture"}


class Failure(Exception):
    pass


def png_size(path: Path) -> tuple[int, int]:
    """Read width/height from the IHDR chunk without a Pillow dependency."""
    with path.open("rb") as handle:
        header = handle.read(24)
    if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
        raise Failure(f"{path.relative_to(ROOT)} is not a PNG")
    return struct.unpack(">II", header[16:24])


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def parse_geometry(value: str, scenario: str) -> tuple[int, int]:
    try:
        width, height = value.lower().split("x")
        return int(width), int(height)
    except (AttributeError, ValueError):
        raise Failure(f"{scenario}: geometry must look like 1600x900, got {value!r}")


def validate() -> None:
    manifest = json.loads(MANIFEST.read_text())
    environment = json.loads(ENVIRONMENT.read_text())
    if not environment.get("openidea", {}).get("source_commit"):
        raise Failure("environment.json must record the OpenIDEA source commit")

    reference = manifest.get("reference", {})
    if reference.get("source_commit") != environment["openidea"]["source_commit"]:
        raise Failure("manifest and environment disagree on the OpenIDEA commit")

    scenarios = manifest.get("scenarios", [])
    if not scenarios:
        raise Failure("manifest has no scenarios")

    seen_ids: set[str] = set()
    seen_hashes: dict[str, str] = {}
    captured = 0

    for index, scenario in enumerate(scenarios, start=1):
        scenario_id = scenario.get("id", "")
        label = scenario_id or f"scenario #{index}"

        if not scenario_id:
            raise Failure(f"{label}: missing id")
        if scenario_id in seen_ids:
            raise Failure(f"{scenario_id}: duplicate id")
        seen_ids.add(scenario_id)

        expected_prefix = f"{index:02d}-"
        if not scenario_id.startswith(expected_prefix):
            raise Failure(f"{scenario_id}: expected numeric prefix {expected_prefix}")

        if scenario.get("status") not in STATUSES:
            raise Failure(f"{scenario_id}: unknown status {scenario.get('status')!r}")

        masks = scenario.get("masks", [])
        if not isinstance(masks, list):
            raise Failure(f"{scenario_id}: masks must be a list")
        for mask in masks:
            if set(mask) != {"x", "y", "width", "height"}:
                raise Failure(f"{scenario_id}: mask must have x/y/width/height, got {mask!r}")

        image = scenario.get("openidea")
        if scenario["status"] == "pending-capture":
            if image and (BASELINES / image).exists():
                raise Failure(
                    f"{scenario_id}: pending scenario ships an image ({image}); "
                    "remove it or capture the real state"
                )
            continue

        if not image:
            raise Failure(f"{scenario_id}: captured scenario needs an 'openidea' path")
        path = BASELINES / image
        if not path.exists():
            raise Failure(f"{scenario_id}: missing capture {image}")

        if "geometry" not in scenario:
            raise Failure(f"{scenario_id}: captured scenario needs geometry")
        width, height = parse_geometry(scenario["geometry"], scenario_id)
        real_width, real_height = png_size(path)
        if (width, height) != (real_width, real_height):
            raise Failure(
                f"{scenario_id}: geometry says {width}x{height} but {image} is "
                f"{real_width}x{real_height}"
            )

        image_hash = digest(path)
        if image_hash in seen_hashes:
            raise Failure(
                f"{scenario_id}: identical pixels to {seen_hashes[image_hash]}; "
                "a copied capture is not a baseline"
            )
        seen_hashes[image_hash] = scenario_id
        captured += 1

    pending = len(scenarios) - captured
    print(
        f"manifest ok: {len(scenarios)} scenarios, {captured} captured, "
        f"{pending} pending; reference {reference.get('source_commit', '?')[:7]}"
    )
    if pending:
        print("pending: " + ", ".join(s["id"] for s in scenarios if s["status"] == "pending-capture"))


def main() -> int:
    try:
        validate()
    except Failure as error:
        print(f"manifest invalid: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
