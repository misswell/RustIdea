#!/usr/bin/env python3
"""Generate Rust design tokens from the pinned OpenIDEA tokens.css.

This is the build-time converter required by RUSTIDEA_UI_PARITY.md.  It is a
small, deterministic CSS reader: no browser, no JavaScript runtime.  The
generated file is checked in so an ordinary `cargo build` needs no Python;
`--check` verifies that it is up to date.

Usage:
  python3 tools/openidea-tokens/generate.py
  python3 tools/openidea-tokens/generate.py --check
"""
from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import sys

try:
    import tinycss2
except ImportError:  # pragma: no cover - CI installs it in the token job
    sys.exit("tinycss2 is required: pip install tinycss2")

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "third_party/openidea/tokens.css"
OUTPUT = ROOT / "crates/rustidea-design/src/tokens_generated.rs"

GENERIC_ROOT = re.compile(r"^:root$")
LIGHT = re.compile(r'^:root\[data-theme="light"\]$')
DARK_HC = re.compile(r'^:root\[data-theme="dark-hc"\]$')
DARK_HIGH_CONTRAST = re.compile(r'^:root\[data-theme="dark"\]\[data-contrast="high"\]$')
COMPACT = re.compile(r"^:root\[data-compact\]$")
HTML = re.compile(r"^html$")
COLOR_MIX = re.compile(r"color-mix\(in srgb,\s*(.*?),\s*(.*?)\)", re.DOTALL)


def split_selectors(prelude: str) -> list[str]:
    # Selector lists here are simple; split commas outside :not(...) parentheses.
    out, buf, depth = [], [], 0
    for ch in prelude:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth = max(0, depth - 1)
        if ch == "," and depth == 0:
            out.append("".join(buf).strip())
            buf = []
        else:
            buf.append(ch)
    if buf:
        out.append("".join(buf).strip())
    return [re.sub(r"\s+", " ", s) for s in out if s]


def declarations(css: str):
    """Yield (selector_list, [(name, value)]) in source order."""
    sheet = tinycss2.parse_stylesheet(css, skip_comments=True, skip_whitespace=True)
    for rule in sheet:
        if rule.type != "qualified-rule":
            continue
        selectors = split_selectors(tinycss2.serialize(rule.prelude).strip())
        decls: list[tuple[str, str]] = []
        for d in tinycss2.parse_declaration_list(rule.content):
            if d.type != "declaration" or not d.name.startswith("--"):
                continue
            value = tinycss2.serialize(d.value).strip()
            value = re.sub(r"\s+", " ", value)
            decls.append((d.name, value))
        if decls:
            yield selectors, decls


def applies(selectors: list[str], variant: str) -> bool:
    for s in selectors:
        if HTML.match(s) or GENERIC_ROOT.match(s):
            return True
        if variant == "light" and LIGHT.match(s):
            return True
        if variant == "dark-hc" and (DARK_HC.match(s) or DARK_HIGH_CONTRAST.match(s)):
            return True
    return False


def collect(css: str) -> dict[str, dict[str, str]]:
    themes: dict[str, dict[str, str]] = {"dark": {}, "light": {}, "dark-hc": {}}
    compact: dict[str, str] = {}
    for selectors, decls in declarations(css):
        for variant in themes:
            if applies(selectors, variant):
                for name, value in decls:
                    themes[variant][name] = value
        if any(COMPACT.match(s) for s in selectors):
            for name, value in decls:
                compact[name] = value
    themes["dark-compact"] = {**themes["dark"], **compact}
    themes["light-compact"] = {**themes["light"], **compact}
    themes["dark-hc-compact"] = {**themes["dark-hc"], **compact}
    return themes


HEX = re.compile(r"^#([0-9a-fA-F]{3,8})$")
RGBA = re.compile(r"^rgba?\(([^)]+)\)$")
NAMED = {
    "transparent": (0, 0, 0, 0.0),
    "black": (0, 0, 0, 1.0),
    "white": (255, 255, 255, 1.0),
}


def parse_color(value: str):
    value = value.strip()
    if value in NAMED:
        return NAMED[value]
    m = HEX.match(value)
    if m:
        h = m.group(1)
        if len(h) in (3, 4):
            h = "".join(c * 2 for c in h)
        if len(h) == 6:
            h += "ff"
        if len(h) == 8:
            return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), int(h[6:8], 16) / 255.0)
    m = RGBA.match(value)
    if m:
        parts = [p.strip() for p in m.group(1).replace("/", " ").split(",")]
        if len(parts) == 3:
            parts.append("1")
        if len(parts) == 4:
            rgb = []
            for p in parts[:3]:
                rgb.append(round(float(p[:-1]) * 2.55) if p.endswith("%") else int(float(p)))
            a = parts[3]
            alpha = float(a[:-1]) / 100 if a.endswith("%") else float(a)
            return (*rgb, alpha)
    return None


def color_to_css(color) -> str:
    r, g, b, a = color
    if a >= 0.999:
        return f"#{r:02x}{g:02x}{b:02x}"
    return f"rgba({r}, {g}, {b}, {a:.3f})".replace("0.", ".")


def find_var_expr(text: str):
    """Return (start, end, name, fallback) of the first var(...), respecting nesting."""
    i = text.find("var(")
    if i < 0:
        return None
    depth, j = 0, i + 4
    while j < len(text):
        if text[j] == "(":
            depth += 1
        elif text[j] == ")":
            if depth == 0:
                break
            depth -= 1
        j += 1
    if j >= len(text):
        return None
    inner = text[i + 4 : j]
    depth, comma = 0, -1
    for k, ch in enumerate(inner):
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        elif ch == "," and depth == 0:
            comma = k
            break
    if comma >= 0:
        return i, j + 1, inner[:comma].strip(), inner[comma + 1 :].strip()
    return i, j + 1, inner.strip(), None


def resolve(text: str, table: dict[str, str], seen: set[str] | None = None) -> str:
    seen = seen or set()
    value = text.strip()
    for _ in range(64):
        v = find_var_expr(value)
        if v is None:
            break
        start, end, name, fallback = v
        if name in seen:
            break
        replacement = table.get(name)
        if replacement is None:
            if fallback is None:
                break
            replacement = fallback
        replacement = resolve(replacement, table, seen | {name})
        value = value[:start] + replacement + value[end:]
    # Resolve color-mix() after its arguments are concrete.
    for _ in range(32):
        m = COLOR_MIX.search(value)
        if not m:
            break
        a_expr, b_expr = m.group(1), m.group(2)
        def split_weight(expr: str):
            parts = expr.rsplit(" ", 1)
            if len(parts) == 2 and parts[1].endswith("%"):
                try:
                    return parts[0].strip(), float(parts[1][:-1]) / 100
                except ValueError:
                    pass
            return expr.strip(), None
        a_raw, a_w = split_weight(a_expr)
        b_raw, b_w = split_weight(b_expr)
        a, b = parse_color(a_raw), parse_color(b_raw)
        if not a or not b:
            break
        if a_w is None and b_w is None:
            a_w, b_w = 0.5, 0.5
        elif a_w is None:
            a_w = 1 - b_w
        elif b_w is None:
            b_w = 1 - a_w
        total = a_w + b_w
        if total <= 0:
            break
        a_w, b_w = a_w / total, b_w / total
        # color-mix() premultiplies alpha, as CSS does.
        alpha = a[3] * a_w + b[3] * b_w
        if alpha <= 0:
            mixed = (0, 0, 0, 0.0)
        else:
            rgb = tuple(
                round((a[i] * a[3] * a_w + b[i] * b[3] * b_w) / alpha) for i in range(3)
            )
            mixed = (*rgb, alpha)
        value = value[: m.start()] + color_to_css(mixed) + value[m.end() :]
    return value


def rust_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    css = SOURCE.read_text()
    themes = collect(css)
    resolved = {
        variant: {name: resolve(value, table) for name, value in table.items()}
        for variant, table in themes.items()
    }
    # Keep insertion order (source order) and drop declarations that are identical
    # to the resolved dark theme from the compact variants? No: each variant is
    # emitted complete so runtime lookup is a single table.
    source_hash = hashlib.sha256(css.encode()).hexdigest()
    lines = [
        "// @generated by tools/openidea-tokens/generate.py from",
        "// third_party/openidea/tokens.css (OpenIDEA fe3e647, Apache-2.0).",
        "// Do not edit by hand; run the generator and commit the result.",
        f'pub const SOURCE_SHA256: &str = "{source_hash}";',
        "",
    ]
    for variant in ("dark", "light", "dark-hc", "dark-compact", "light-compact", "dark-hc-compact"):
        table = resolved[variant]
        const = variant.upper().replace("-", "_")
        lines.append(f"pub static {const}: &[(&str, &str)] = &[")
        for name, value in sorted(table.items()):
            lines.append(f'    ("{rust_escape(name)}", "{rust_escape(value)}"),')
        lines.append("];")
        lines.append("")
    output = "\n".join(lines)
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != output:
            print(f"{OUTPUT} is out of date; run tools/openidea-tokens/generate.py", file=sys.stderr)
            return 1
        print("design tokens are up to date")
        return 0
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(output)
    print(f"wrote {OUTPUT.relative_to(ROOT)} ({len(output)} bytes, {len(resolved['dark'])} dark tokens)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
