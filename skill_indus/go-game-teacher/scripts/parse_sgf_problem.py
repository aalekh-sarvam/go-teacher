#!/usr/bin/env python3
"""Parse an SGF go problem (setup + solution variations) into puzzle JSON.

Typical inputs: gogameguru/go-problems weekly problems, tasuki2sgf collections,
OGS/GoGui exports. The root node carries the setup (AB/AW, optional PL, SZ,
C); the variations hold the solution tree.

Classification of variations (leaf-to-leaf paths through the tree):
  * a leaf whose last-node comment matches a WRONG marker ("wrong",
    "incorrect", "fail", "bad", "✗", "not correct") is wrong;
  * otherwise a last-node comment matching a CORRECT marker ("correct",
    "right", "✓", "RIGHT") is correct;
  * if no leaf in the whole tree carries a marker, the FIRST variation at the
    first branch point is correct and every other one is wrong
    (the usual convention of hand-made problem files).
A first move is `correct` when at least one leaf below it is correct. Its
`correct_line` is the longest correct path through it. Every other first move
is a `wrong_move` whose refutation is the longest path below it.

Output (same shape as parse_tasuki_tex.py, plus solution fields):
  { "title", "board_size", "player_to_move": "B"|"W",
    "black": [GTP...], "white": [GTP...],
    "sgf_black": [...], "sgf_white": [...], "labels": [...],
    "description": str,
    "correct_moves": [GTP...], "correct_lines": {GTP: [GTP...]},
    "wrong_moves": [{"move", "refutation": [GTP...], "explanation"}],
    "warnings": [...],
    "source": {"file", "problem", "example_locator",
               "position": {board_size, black_stones, white_stones, player_to_move}} }

Coordinates: SGF letters a..s (a = column 0 = row 0, row 'a' at the TOP,
`tt` or an empty value is a pass). GTP columns A-T skip the letter I and rows
count from the BOTTOM, so SGF `da` on 19x19 is GTP D19.

Usage:
  python parse_sgf_problem.py problem.sgf -o puzzle.json
  python parse_sgf_problem.py collection.sgf --index 3 -o puzzle.json
  python parse_sgf_problem.py collection.sgf --all -o all.json

A parsed problem is a *source position*: it still needs the transformation,
independent verification (solve_tsumego.py / verify_puzzle_engine.py) and
objective fields required by references/grounding-and-practice.md.
"""

import argparse
import json
import re
import sys

GTP_COLS = "ABCDEFGHJKLMNOPQRSTUVWXYZ"  # no 'I'

WRONG_RE = re.compile(r"(\bwrong\b|\bincorrect\b|\bfail(s|ed|ure)?\b|\bbad\b|✗|\bnot\s+correct\b)", re.I)
CORRECT_RE = re.compile(r"(\bcorrect\b|\bright\b|✓|\bsuccess\b|\bsolution\b)", re.I)


# --------------------------------------------------------------------------
# coordinates
# --------------------------------------------------------------------------

def sgf_to_gtp(sgf, size, size_y=None):
    """SGF point (e.g. 'da') -> GTP ('D19' on 19x19). Passes -> 'pass'."""
    size_y = size_y or size
    if sgf == "" or (sgf == "tt" and size <= 19 and size_y <= 19):
        return "pass"
    if len(sgf) != 2:
        raise ValueError(f"bad SGF point {sgf!r}")
    col, row = ord(sgf[0]) - 97, ord(sgf[1]) - 97
    if not (0 <= col < size and 0 <= row < size_y):
        raise ValueError(f"SGF point {sgf!r} outside {size}x{size_y} board")
    return f"{GTP_COLS[col]}{size_y - row}"


def gtp_to_sgf(gtp, size, size_y=None):
    size_y = size_y or size
    if gtp == "pass":
        return "tt" if size <= 19 else ""
    col = GTP_COLS.index(gtp[0])
    row = size_y - int(gtp[1:])
    return chr(col + 97) + chr(row + 97)


def expand_points(value):
    """Expand a compressed SGF point list 'aa:cc' into individual points."""
    if ":" in value:
        a, b = value.split(":", 1)
        c0, r0, c1, r1 = ord(a[0]), ord(a[1]), ord(b[0]), ord(b[1])
        return [chr(c) + chr(r) for c in range(min(c0, c1), max(c0, c1) + 1)
                for r in range(min(r0, r1), max(r0, r1) + 1)]
    return [value]


# --------------------------------------------------------------------------
# SGF tokenizer / tree builder
# --------------------------------------------------------------------------

class Node:
    __slots__ = ("props", "children")

    def __init__(self):
        self.props = {}
        self.children = []

    def get(self, key, default=None):
        vals = self.props.get(key)
        return vals[0] if vals else default


def _unescape(text):
    return re.sub(r"\\(.)", r"\1", text.replace("\\\r\n", "").replace("\\\n", ""))


def parse_sgf(text):
    """Return a list of game trees (root Nodes) from an SGF string/collection."""
    n = len(text)
    pos = [0]
    games = []

    def skip_ws():
        while pos[0] < n and text[pos[0]].isspace():
            pos[0] += 1

    def parse_node():
        node = Node()
        while True:
            skip_ws()
            m = re.match(r"[A-Za-z]+", text[pos[0]:])
            if not m:
                return node
            ident = "".join(c for c in m.group(0) if c.isupper()) or m.group(0)
            pos[0] += m.end()
            values = []
            while True:
                skip_ws()
                if pos[0] >= n or text[pos[0]] != "[":
                    break
                j = pos[0] + 1
                buf = []
                while j < n and text[j] != "]":
                    if text[j] == "\\" and j + 1 < n:
                        buf.append(text[j:j + 2]); j += 2
                    else:
                        buf.append(text[j]); j += 1
                if j >= n:
                    raise ValueError("unterminated property value")
                values.append(_unescape("".join(buf)))
                pos[0] = j + 1
            node.props.setdefault(ident, []).extend(values)

    def parse_gametree(parent):
        """'(' already consumed; attach the sequence to parent (None = new game)."""
        current = parent
        while True:
            skip_ws()
            if pos[0] >= n:
                raise ValueError("unbalanced '(' in SGF")
            ch = text[pos[0]]
            if ch == ";":
                pos[0] += 1
                node = parse_node()
                if current is None:
                    games.append(node)
                else:
                    current.children.append(node)
                current = node
            elif ch == "(":
                pos[0] += 1
                parse_gametree(current)
            elif ch == ")":
                pos[0] += 1
                return
            else:
                pos[0] += 1  # stray character

    while True:
        skip_ws()
        if pos[0] >= n:
            break
        if text[pos[0]] == "(":
            pos[0] += 1
            parse_gametree(None)
        elif text[pos[0]] == ")":
            raise ValueError("unbalanced ')' in SGF")
        else:
            pos[0] += 1
    return games


# --------------------------------------------------------------------------
# problem extraction
# --------------------------------------------------------------------------

def _leaf_paths(node, path, out):
    """Collect (moves, last_comment, leaf_node) for every leaf below node.

    `node` itself is NOT part of the path (call with the setup node)."""
    if not node.children:
        out.append((list(path), node.get("C", ""), node))
        return
    for child in node.children:
        mv = None
        for color in ("B", "W"):
            if color in child.props:
                mv = (color, child.props[color][0])
        path.append((mv, child))
        _leaf_paths(child, path, out)
        path.pop()


def classify_comment(comment):
    if not comment:
        return None
    if WRONG_RE.search(comment):
        return "wrong"
    if CORRECT_RE.search(comment):
        return "correct"
    return None


def problem_from_tree(root, size_hint=None, locator="", index=0):
    size_prop = root.get("SZ", str(size_hint or 19))
    if ":" in size_prop:
        sx, sy = (int(v) for v in size_prop.split(":", 1))
    else:
        sx = sy = int(size_prop)
    if not (2 <= sx <= 25 and 2 <= sy <= 25):
        raise ValueError(f"unsupported board size {size_prop}")

    def conv(pt):
        return sgf_to_gtp(pt, sx, sy)

    black_sgf = [p for v in root.props.get("AB", []) for p in expand_points(v)]
    white_sgf = [p for v in root.props.get("AW", []) for p in expand_points(v)]
    # setup nodes sometimes span root + first child with no move; merge those
    setup_node = root
    while (len(setup_node.children) == 1 and not any(k in setup_node.children[0].props for k in ("B", "W"))
           and any(k in setup_node.children[0].props for k in ("AB", "AW", "PL", "C"))):
        setup_node = setup_node.children[0]
        black_sgf += [p for v in setup_node.props.get("AB", []) for p in expand_points(v)]
        white_sgf += [p for v in setup_node.props.get("AW", []) for p in expand_points(v)]
    erased = {p for v in setup_node.props.get("AE", []) for p in expand_points(v)}
    black_sgf = [p for p in black_sgf if p not in erased]
    white_sgf = [p for p in white_sgf if p not in erased]

    labels = []
    for v in root.props.get("LB", []) + setup_node.props.get("LB", []):
        if ":" in v:
            pt, lab = v.split(":", 1)
            labels.append({"point": conv(pt), "label": lab})

    leaves = []
    _leaf_paths(setup_node, [], leaves)
    leaves = [(mvs, c, n) for mvs, c, n in leaves if mvs]  # ignore an empty tree

    warnings = []
    # player to move
    pl = root.get("PL") or setup_node.get("PL")
    if pl:
        player = "B" if pl.upper().startswith("B") or pl == "1" else "W"
    elif leaves and leaves[0][0][0][0]:
        player = leaves[0][0][0][0][0]
    else:
        player = "B"
        warnings.append("no PL and no moves: player_to_move defaulted to B")

    # classify leaves
    labelled = [(mvs, classify_comment(c), c) for mvs, c, _ in leaves]
    any_marker = any(cls for _, cls, _ in labelled)
    if not any_marker and leaves:
        warnings.append("no correct/wrong markers in comments: first variation assumed correct")
        # first child at the first real branch point (may be deeper than ply 1)
        fork = setup_node
        while len(fork.children) == 1:
            fork = fork.children[0]
        first_branch = fork.children[0] if fork.children else None
        labelled = [(mvs, "correct" if first_branch is None or any(n is first_branch for _, n in mvs) else "wrong", c)
                    for mvs, _, c in labelled]
    else:
        labelled = [(mvs, cls or "wrong", c) for mvs, cls, c in labelled]

    def to_line(mvs):
        line, expect = [], player
        for (mv, _node) in mvs:
            if mv is None:
                continue  # comment-only node
            color, pt = mv
            if color != expect:
                warnings.append(f"non-alternating colours in line {[m[0][1] for m in mvs if m[0]]}")
            line.append(conv(pt))
            expect = "W" if color == "B" else "B"
        return line

    by_first = {}
    order = []
    for mvs, cls, comment in labelled:
        line = to_line(mvs)
        if not line:
            continue
        first = line[0]
        if first not in by_first:
            by_first[first] = []
            order.append(first)
        by_first[first].append((cls, line, comment))

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
            wrong_moves.append({
                "move": first,
                "refutation": best[1][1:],
                "explanation": best[2].strip(),
            })

    title = root.get("GN") or root.get("EV") or f"problem {index + 1}"
    black = [conv(p) for p in black_sgf]
    white = [conv(p) for p in white_sgf]
    return {
        "title": title,
        "board_size": sx,
        **({"board_size_y": sy} if sy != sx else {}),
        "player_to_move": player,
        "black": black,
        "white": white,
        "sgf_black": black_sgf,
        "sgf_white": white_sgf,
        "labels": labels,
        "description": (root.get("C") or setup_node.get("C") or "").strip(),
        "correct_moves": correct_moves,
        "correct_lines": correct_lines,
        "wrong_moves": wrong_moves,
        "warnings": sorted(set(warnings)),
        "source": {
            "file": locator,
            "problem": index + 1,
            "example_locator": f"{locator} problem {index + 1}" if locator else f"problem {index + 1}",
            "position": {
                "board_size": sx,
                "black_stones": black,
                "white_stones": white,
                "player_to_move": player,
            },
        },
    }


def parse_sgf_problems(text, locator="", size_hint=None):
    games = parse_sgf(text)
    return [problem_from_tree(g, size_hint, locator, i) for i, g in enumerate(games)]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("input", help="SGF file (single problem or collection)")
    ap.add_argument("--index", type=int, default=0, help="0-based game index within a collection")
    ap.add_argument("--all", action="store_true", help="emit every game as a JSON list")
    ap.add_argument("--size", type=int, help="board size when SZ is missing (default 19)")
    ap.add_argument("-o", "--output", help="output JSON path (default: stdout)")
    args = ap.parse_args()

    with open(args.input, encoding="utf-8", errors="replace") as f:
        text = f.read()
    problems = parse_sgf_problems(text, args.input, args.size)
    if not problems:
        sys.exit("no game trees found")
    if args.all:
        result = problems
    else:
        if not 0 <= args.index < len(problems):
            sys.exit(f"--index {args.index} out of range (file has {len(problems)} games)")
        result = problems[args.index]
        for w in result["warnings"]:
            print(f"warning: {w}", file=sys.stderr)
    data = json.dumps(result, indent=1, ensure_ascii=False)
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(data)
        print(f"{len(problems) if args.all else 1} problem(s) -> {args.output}", file=sys.stderr)
    else:
        print(data)


if __name__ == "__main__":
    main()
