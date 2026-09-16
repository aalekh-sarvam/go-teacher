#!/usr/bin/env python3
"""Parse tasuki/tsumego .tex problem files into machine-readable JSON.

Source files: https://github.com/tasuki/tsumego  (books/problems/*.tex)
Verified against Seon82/tasuki2sgf's generated SGFs: 4,340 of 4,341 problems
match exactly (stones and player to move); lee-chang-ho problem 620 is parsed
*more* correctly here because the shipped SGFs mishandle one labelled-point
macro - see references/tsumego-source-formats.md.

Usage:
  python parse_tasuki_tex.py cho-1.tex -o cho-1.json
  python parse_tasuki_tex.py https://raw.githubusercontent.com/tasuki/tsumego/master/books/problems/cho-1.tex

Output: JSON list of
  { "title": str, "player_to_move": "B"|"W",
    "black": [GTP...], "white": [GTP...],
    "sgf_black": [SGF...], "sgf_white": [SGF...],
    "labels": [{"point": GTP, "label": str}],
    "source": {"book": str, "problem": int} }

GTP coordinates use letters A-T (no I), rows numbered from the bottom.
SGF coordinates are top-left origin (first letter = column, second = row);
the first tex line of a problem is its TOP row.
"""

import argparse
import json
import re
import sys
import urllib.request

GTP_COLS = "ABCDEFGHJKLMNOPQRST"  # 19 columns, no 'I'
BOARD = 19

BLOCK_RE = re.compile(r"^\\vbox\{\\vbox\{\\goo\n([\s\S]*?)\}", re.MULTILINE)
TITLE_RE = re.compile(r"^\\hfil(.*)\\hfil", re.MULTILINE)


def parse_row(row):
    """Parse one tex board line into (stones, labels, width).

    Cell macros (see references/tsumego-source-formats.md):
      \\- @G  black stone on the point whose grid glyph is G
      \\- !G  white stone on that point
      \\0??G  empty point with grid glyph G
      \\0LLG  empty point with 2-char label LL (e.g. \\001( ) and grid glyph G
      \\!  L  empty point carrying label letter L (the label IS the point)
    Each cell occupies exactly one column; stones never consume a column
    by themselves (they ride on their glyph).
    """
    stones = []  # (color, col)
    labels = []  # (label, col)
    col = 0
    i = 0
    while i < len(row):
        if row.startswith(r"\- ", i):
            stone = row[i + 3]
            if stone not in "@!":
                raise ValueError(f"unexpected stone char {stone!r} after '\\- '")
            glyph = row[i + 4]  # the stone rides on this glyph's point
            stones.append((stone, col))
            i += 5
        elif row.startswith(r"\!  ", i):
            label = row[i + 4]
            labels.append((label, col))
            i += 5
        elif row.startswith(r"\0", i):
            label = row[i + 2:i + 4]
            glyph = row[i + 4]
            if label != "??":
                labels.append((label, col))
            i += 5
        else:
            # Bare glyph with no macro prefix should not appear in the raw
            # tex; if it does, treat it as one point (defensive).
            glyph = row[i]
            i += 1
        col += 1
    return stones, labels, col


def sgf_pt(row_idx, col_idx):
    return chr(col_idx + 97) + chr(row_idx + 97)


def gtp_pt(row_idx, col_idx):
    return f"{GTP_COLS[col_idx]}{BOARD - row_idx}"


def parse_tex(tex, book_name=""):
    problems = []
    blocks = list(BLOCK_RE.finditer(tex))
    titles = [m.group(1).strip() for m in TITLE_RE.finditer(tex)]
    for n, match in enumerate(blocks):
        title = titles[n] if n < len(titles) else f"problem {n + 1}"
        black, white, labels = [], [], []
        for row_idx, raw_row in enumerate(match.group(1).split("\n")):
            row = raw_row.strip()  # strips stray U+3000 etc.
            if not row:
                continue
            row_stones, row_labels, width = parse_row(row)
            if width > BOARD:
                raise ValueError(
                    f"{book_name} '{title}': tex line {row_idx} has {width} "
                    f"points, expected {BOARD} - unhandled macro?"
                )
            if width < BOARD:
                # A row shorter than the board can only be missing empty
                # trailing cells; stones can never sit beyond column 18 in
                # that case. (Observed 0 times after whitespace stripping.)
                print(f"warning: {book_name} '{title}' line {row_idx}: "
                      f"{width} points (missing trailing empty cells)",
                      file=sys.stderr)
            for color, col in row_stones:
                (black if color == "@" else white).append((row_idx, col))
            for label, col in row_labels:
                labels.append({"point": gtp_pt(row_idx, col), "label": label})
        problems.append({
            "title": title,
            "player_to_move": "W" if "white to play" in title.lower() else "B",
            "black": [gtp_pt(r, c) for r, c in black],
            "white": [gtp_pt(r, c) for r, c in white],
            "sgf_black": [sgf_pt(r, c) for r, c in black],
            "sgf_white": [sgf_pt(r, c) for r, c in white],
            "labels": labels,
            "source": {"book": book_name, "problem": n + 1},
        })
    return problems


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("input", help=".tex file path or http(s) URL")
    ap.add_argument("-o", "--output", help="output JSON path (default: stdout)")
    ap.add_argument("--book", default="", help="book name for the source field")
    args = ap.parse_args()

    if args.input.startswith(("http://", "https://")):
        tex = urllib.request.urlopen(args.input, timeout=60).read().decode("utf-8")
    else:
        tex = open(args.input, encoding="utf-8").read()

    book = args.book or re.sub(r"\.tex$", "", args.input.rsplit("/", 1)[-1])
    problems = parse_tex(tex, book)
    data = json.dumps(problems, indent=1)

    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(data)
        print(f"{len(problems)} problems -> {args.output}", file=sys.stderr)
    else:
        print(data)


if __name__ == "__main__":
    main()
