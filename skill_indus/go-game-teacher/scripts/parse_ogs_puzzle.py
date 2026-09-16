#!/usr/bin/env python3
"""Convert an OGS puzzle (online-go.com) into puzzle JSON.

Input: an OGS puzzle id (fetched from https://online-go.com/api/v1/puzzles/<id>)
or a local JSON file saved from that endpoint.

Verified shape (puzzle 45, 2026-09):
  top level: id, name, collection{id,name,...}, owner{username,...}, private,
             rating, type ("puzzle"), puzzle{...}
  puzzle: puzzle_rank (string, "0" = unranked?), puzzle_type ("life_and_death"...),
          width, height, initial_state{black, white} as concatenated letter
          pairs (two letters per stone, 'a' = 0, x first then y, y counted
          from the TOP), initial_player ("black"|"white"), puzzle_description,
          move_tree: {x:-1, y:-1, branches:[{x, y, correct_answer?: true,
                      wrong_answer?: true, text?, marks?, branches:[...]}]}

Conversion:
  stones -> GTP (columns skip 'I', rows from the bottom: x=3,y=0 on 19x19 = D19)
  correct_moves  = first-ply branches flagged correct_answer or with a
                   correct_answer node somewhere below them
  correct_lines  = the deepest correct path under each correct first move
  wrong_moves    = every other first-ply branch; refutation = deepest
                   continuation, explanation = the branch/leaf `text`
  rank_hint      = raw puzzle_rank plus a tentative label (unconfirmed)
  source.url     = the API URL; example_locator = "OGS puzzle <id> (<name>)"

Output shape matches parse_sgf_problem.py / parse_tasuki_tex.py.

Usage:
  python parse_ogs_puzzle.py 45 -o puzzle.json
  python parse_ogs_puzzle.py saved_puzzle_45.json -o puzzle.json

OGS puzzles are user-contributed. Credit the owner and OGS, check the
collection's terms before redistributing, and never present a puzzle as
verified until solve_tsumego.py / verify_puzzle_engine.py has checked the
transformed board (references/grounding-and-practice.md).
"""

import argparse
import json
import os
import sys
import urllib.request

try:
    from parse_sgf_problem import GTP_COLS, gtp_to_sgf
except ImportError:  # running from another cwd without scripts/ on sys.path
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from parse_sgf_problem import GTP_COLS, gtp_to_sgf

API_URL = "https://online-go.com/api/v1/puzzles/{id}"
WEB_URL = "https://online-go.com/puzzle/{id}"


# --------------------------------------------------------------------------
# coordinates
# --------------------------------------------------------------------------

def xy_to_gtp(x, y, width, height):
    if x < 0 or y < 0:
        return "pass"
    if not (x < width and y < height):
        raise ValueError(f"OGS point ({x},{y}) outside {width}x{height}")
    return f"{GTP_COLS[x]}{height - y}"


def letters_to_gtp(letters, width, height):
    """'abcd' -> [GTP(x=0,y=1), GTP(x=2,y=3)]."""
    letters = letters or ""
    if len(letters) % 2:
        raise ValueError(f"odd-length stone string {letters!r}")
    return [xy_to_gtp(ord(letters[i]) - 97, ord(letters[i + 1]) - 97, width, height)
            for i in range(0, len(letters), 2)]


# --------------------------------------------------------------------------
# move tree
# --------------------------------------------------------------------------

def _paths(node, path, out):
    """Collect (moves, flags, texts) for each leaf under node (node excluded)."""
    branches = node.get("branches") or []
    if not branches:
        out.append((list(path),
                    [n.get("correct_answer") for n in path],
                    [n.get("wrong_answer") for n in path],
                    [n.get("text") or "" for n in path]))
        return
    for child in branches:
        path.append(child)
        _paths(child, path, out)
        path.pop()


def rank_hint(raw):
    """Best-effort reading of OGS puzzle_rank. Semantics NOT confirmed."""
    hint = {"puzzle_rank": raw,
            "note": ("OGS puzzle_rank as stored by the API; '0' appears to mean "
                     "unranked. Interpretation unconfirmed - do not quote as a "
                     "difficulty claim without checking OGS.")}
    try:
        r = int(raw)
    except (TypeError, ValueError):
        return hint
    if r <= 0:
        hint["tentative_label"] = "unranked"
    elif r < 30:
        hint["tentative_label"] = f"{30 - r}k (if OGS rank numbering)"
    else:
        hint["tentative_label"] = f"{r - 29}d (if OGS rank numbering)"
    return hint


def convert(data, url=None):
    puzzle = data.get("puzzle") or {}
    if not puzzle:
        raise ValueError("no 'puzzle' object in OGS JSON")
    width = int(puzzle.get("width", 19))
    height = int(puzzle.get("height", width))
    init = puzzle.get("initial_state") or {}
    black = letters_to_gtp(init.get("black", ""), width, height)
    white = letters_to_gtp(init.get("white", ""), width, height)
    player = "W" if str(puzzle.get("initial_player", "black")).lower().startswith("w") else "B"
    pid = data.get("id")
    name = data.get("name") or puzzle.get("name") or f"OGS puzzle {pid}"
    url = url or (API_URL.format(id=pid) if pid is not None else "")

    leaves = []
    _paths(puzzle.get("move_tree") or {}, [], leaves)
    warnings = []

    def conv(node):
        return xy_to_gtp(int(node.get("x", -1)), int(node.get("y", -1)), width, height)

    by_first, order = {}, []
    for nodes, correct_flags, wrong_flags, texts in leaves:
        if not nodes:
            continue
        line = [conv(n) for n in nodes]
        cls = "correct" if any(correct_flags) else "wrong"
        if any(correct_flags) and any(wrong_flags):
            warnings.append(f"line {line} carries both correct_answer and wrong_answer flags")
        text = next((t for t in reversed(texts) if t), "")
        first = line[0]
        if first not in by_first:
            by_first[first] = []
            order.append(first)
        by_first[first].append((cls, line, text))

    correct_moves, correct_lines, wrong_moves = [], {}, []
    for first in order:
        entries = by_first[first]
        good = [e for e in entries if e[0] == "correct"]
        if good:
            best = max(good, key=lambda e: len(e[1]))
            correct_moves.append(first)
            correct_lines[first] = best[1]
        else:
            best = max(entries, key=lambda e: len(e[1]))
            wrong_moves.append({"move": first, "refutation": best[1][1:],
                                "explanation": best[2].strip()})
    if not correct_moves:
        warnings.append("no branch flagged correct_answer")

    owner = (data.get("owner") or {}).get("username")
    collection = data.get("collection") or {}
    return {
        "title": name,
        "board_size": width,
        **({"board_size_y": height} if height != width else {}),
        "player_to_move": player,
        "black": black,
        "white": white,
        "sgf_black": [gtp_to_sgf(p, width, height) for p in black],
        "sgf_white": [gtp_to_sgf(p, width, height) for p in white],
        "labels": [],
        "description": (puzzle.get("puzzle_description") or "").strip(),
        "correct_moves": correct_moves,
        "correct_lines": correct_lines,
        "wrong_moves": wrong_moves,
        "rank_hint": rank_hint(puzzle.get("puzzle_rank")),
        "warnings": warnings,
        "source": {
            "url": url,
            "web_url": WEB_URL.format(id=pid) if pid is not None else "",
            "title": f"OGS puzzle: {name}",
            "example_locator": f"OGS puzzle {pid} ({name})",
            "problem": pid,
            "owner": owner,
            "collection_id": collection.get("id"),
            "collection_name": collection.get("name"),
            "puzzle_type": puzzle.get("puzzle_type"),
            "private": data.get("private"),
            "rating": data.get("rating"),
            "type": data.get("type"),
            "credit": (f"Puzzle by {owner or 'unknown OGS user'} on online-go.com; "
                       "check the collection's terms before reuse."),
            "position": {
                "board_size": width,
                "black_stones": black,
                "white_stones": white,
                "player_to_move": player,
            },
        },
    }


def load(arg):
    if arg.isdigit():
        url = API_URL.format(id=arg)
        req = urllib.request.Request(url, headers={"User-Agent": "go-game-teacher/parse_ogs_puzzle"})
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read().decode("utf-8")), url
    with open(arg, encoding="utf-8") as f:
        return json.load(f), None


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("puzzle", help="OGS puzzle id (fetched) or path to a saved API JSON")
    ap.add_argument("-o", "--output", help="output JSON path (default: stdout)")
    args = ap.parse_args()

    data, url = load(args.puzzle)
    result = convert(data, url)
    for w in result["warnings"]:
        print(f"warning: {w}", file=sys.stderr)
    print("credit: OGS puzzles are user-contributed content. "
          f"{result['source']['credit']} Link {result['source']['web_url'] or result['source']['url']}.",
          file=sys.stderr)

    out = json.dumps(result, indent=1, ensure_ascii=False)
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(out)
        print(f"{result['source']['example_locator']} -> {args.output}", file=sys.stderr)
    else:
        print(out)


if __name__ == "__main__":
    main()
