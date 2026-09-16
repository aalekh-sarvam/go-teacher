#!/usr/bin/env python3
"""Crop a parsed tsumego problem to a small board and verify bounded
capture/survival objectives mechanically, with the skill's own rules engine
(go_rules.Position).

This is the fast path for puzzle seeds. Never verify a tsumego by looking at
diagrams, and never abandon computational verification because a search "felt
slow" - crop the board (default 9x9, the problems already hug the top-left)
and let this tool do the reading. Cropping is a transformation like any
other: the final lesson must still pass validate_lesson.py, which re-checks
objectives with all legal replies.

Inputs come from parse_tasuki_tex.py (a .json file) or directly from a .tex
file / raw GitHub URL.

Find suitable seeds (problems whose target has exactly one verified key move):
  python solve_tsumego.py --json cho-1.json --scan 1:60 --plies 5

Verify one seed (candidates near the target are tested one by one):
  python solve_tsumego.py --json cho-1.json --problem 11 --mode capture --target F19 --plies 5
  python solve_tsumego.py --tex cho-1.tex --problem 31 --mode survive --target B9 --swap-colors -o seed.json

Modes:
  capture  the player to move must force capture of the target group within
           <plies> (objective kind: capture_within)
  survive  the player to move must keep the target group alive for <plies>
           (objective kind: avoid_capture_for)

Search scopes:
  near     "near-fight": each side may play the target group's liberties,
           liberties of groups adjacent to it, and empty points within
           Chebyshev distance 2 of its stones (the set follows the group as
           it extends, so ladders are chased). Fast; used by default for
           --scan. NOT a proof: in validation it produced a false win on
           about 1% of moves (an escape ran outside the restricted set), so
           every seed chosen from a scan must be confirmed in full scope.
  full     all legal replies and pass: exactly the semantics of
           puzzle_reading.capture_within (which validate_lesson.py uses).
           The default for single-problem verification; use it before
           claiming a key move is unique or a move fails.

Coordinates: --target accepts original 19x19 GTP coordinates (e.g. F19) or
cropped-board coordinates (e.g. F9) - the two cannot collide. Verified moves
and the emitted puzzle JSON use the CROPPED board's coordinates. --radius
limits which FIRST moves are tested; increase it before claiming uniqueness.
"""

import argparse
import json
import re
import sys
import urllib.request

from go_rules import Position, COLS
from parse_tasuki_tex import parse_tex


class FastPosition(Position):
    """Position with a cheap clone; the search clones constantly."""

    def clone(self):
        p = FastPosition.__new__(FastPosition)
        p.sx, p.sy = self.sx, self.sy
        p.board = dict(self.board)
        p.to_move, p.rules = self.to_move, self.rules
        p.history = list(self.history)
        return p


def fight_moves(p, anchor):
    """Empty points relevant to the target group's capture fight:
    its liberties, liberties of adjacent groups, and its near points."""
    stones, libs = p.group(anchor)
    opts = set(libs)
    for s in stones:
        for n in p.neighbours(s):
            if n in p.board and p.board[n] != p.board[anchor]:
                _, nl = p.group(n)
                opts |= nl
    for x, y in stones:
        for dx in (-2, -1, 0, 1, 2):
            for dy in (-2, -1, 0, 1, 2):
                pt = (x + dx, y + dy)
                if 0 <= pt[0] < p.sx and 0 <= pt[1] < p.sy and pt not in p.board:
                    opts.add(pt)
    return opts


def bounded_capture(position, target, attacker, plies, node_budget=200000,
                    scope="near"):
    """Bounded capture search over the stated move scope. Returns
    (result, nodes): True = attacker forces capture within plies,
    False = target provably survives the horizon, None = budget exhausted.
    With scope="full" the semantics match puzzle_reading.capture_within
    exactly (all legal board moves and pass, liberties searched first).
    """
    anchor = position.xy(target)
    if anchor not in position.board:
        return True, 0
    if position.board[anchor] == attacker:
        raise ValueError("capture target belongs to attacker")
    memo = {}
    count = [0]

    def options(p):
        if scope == "full":
            _, libs = p.group(anchor)
            points = sorted(libs) + [xy for xy in
                                     ((x, y) for y in range(p.sy) for x in range(p.sx))
                                     if xy not in p.board and xy not in libs]
            return [COLS[x] + str(p.sy - y) for x, y in points]
        return [COLS[x] + str(p.sy - y) for x, y in sorted(fight_moves(p, anchor))]

    def search(p, depth):
        if anchor not in p.board or p.board[anchor] == attacker:
            return True
        if depth <= 0:
            return False
        key = (p.key(), p.to_move, depth)
        if key in memo:
            return memo[key]
        if count[0] >= node_budget:
            return None
        count[0] += 1
        attacking = p.to_move == attacker
        unknown = False
        for mv in options(p) + ["pass"]:
            q = p.clone()
            try:
                q.play(mv)
            except ValueError:
                continue
            r = search(q, depth - 1)
            if r is None:
                unknown = True
            elif r == attacking:
                memo[key] = r
                return r
        result = None if unknown else (not attacking)
        memo[key] = result
        return result

    return search(position, plies), count[0]


def load_problems(source):
    if source.startswith(("http://", "https://")):
        tex = urllib.request.urlopen(source, timeout=60).read().decode("utf-8")
        name = re.sub(r"\.tex$", "", source.rsplit("/", 1)[-1])
        return parse_tex(tex, name)
    if source.endswith(".json"):
        with open(source, encoding="utf-8") as f:
            return json.load(f)
    tex = open(source, encoding="utf-8").read()
    name = re.sub(r"\.tex$", "", source.rsplit("/", 1)[-1])
    return parse_tex(tex, name)


def crop_problem(problem, size, swap_colors=False):
    """Keep the top-left size x size region; remap GTP coordinates."""
    black = problem["white"] if swap_colors else problem["black"]
    white = problem["black"] if swap_colors else problem["white"]
    player = problem["player_to_move"]
    if swap_colors:
        player = "W" if player == "B" else "B"

    def remap(stones):
        kept, dropped = [], []
        for mv in stones:
            x, y = COLS.index(mv[0]), 19 - int(mv[1:])
            if x < size and y < size:
                kept.append(COLS[x] + str(size - y))
            else:
                dropped.append(mv)
        return kept, dropped

    nb, db = remap(black)
    nw, dw = remap(white)
    if db or dw:
        raise ValueError(
            f"stones outside the top-left {size}x{size} crop: "
            f"{sorted(db + dw)} - use a larger --size or another problem"
        )
    return Position(size, black=nb, white=nw, to_move=player), player


def resolve_target(point, size):
    """Accept a target in original 19x19 GTP coordinates or in cropped-board
    coordinates (they cannot collide: original in-crop points have rows
    19-size+1..19, crop points have rows 1..size). Returns (original, crop)."""
    if point[0] not in COLS:
        raise ValueError(f"bad target {point}")
    x, y = COLS.index(point[0]), 19 - int(point[1:])
    if x < size and y < size:
        return point, COLS[x] + str(size - y)
    row = int(point[1:])
    if row <= size:
        cx = COLS.index(point[0])
        return COLS[cx] + str(19 - row), point
    raise ValueError(f"target {point} is outside the top-left {size}x{size} crop")


def gtp(pos, pt):
    return COLS[pt[0]] + str(pos.sy - pt[1])


def groups_of(pos, color):
    seen, out = set(), []
    for pt, c in pos.board.items():
        if c == color and pt not in seen:
            stones, libs = pos.group(pt)
            seen |= stones
            out.append({
                "anchor": gtp(pos, min(stones)),
                "stones": stones, "liberties": libs,
            })
    return out


def candidate_moves(pos, stones, radius):
    pts = set()
    for x, y in stones:
        for dx in range(-radius, radius + 1):
            for dy in range(-radius, radius + 1):
                nx, ny = x + dx, y + dy
                if 0 <= nx < pos.sx and 0 <= ny < pos.sy and (nx, ny) not in pos.board:
                    pts.add((nx, ny))
    return [gtp(pos, pt) for pt in sorted(pts)]


def test_move(pos, move, target, attacker, plies, budget, mode, scope):
    q = pos.clone()
    try:
        q.play(move)
    except ValueError:
        return "illegal", 0
    anchor = pos.xy(target)
    if mode == "capture" and anchor not in q.board:
        return "win", 0
    r, nodes = bounded_capture(q, target, attacker, plies - 1, budget, scope)
    if r is None:
        return "unknown", nodes
    if mode == "capture":
        return ("win" if r else "fail"), nodes
    return ("win" if r is False else "fail"), nodes


def board_stones(pos):
    black = sorted(gtp(pos, pt) for pt, c in pos.board.items() if c == "B")
    white = sorted(gtp(pos, pt) for pt, c in pos.board.items() if c == "W")
    return black, white


def verify_single(problem, args):
    pos, player = crop_problem(problem, args.size, args.swap_colors)
    mode = args.mode
    enemy = "W" if player == "B" else "B"
    attacker = enemy if mode == "survive" else player
    target_color = enemy if mode == "capture" else player

    if not args.target:
        listing = ", ".join(
            f"{g['anchor']} ({target_color}, {len(g['stones'])} stones, "
            f"{len(g['liberties'])} libs)" for g in groups_of(pos, target_color))
        raise SystemExit("--target is required (original 19x19 GTP). "
                         f"{target_color} groups on the crop: {listing or 'none'}")

    original, target = resolve_target(args.target, args.size)
    anchor = pos.xy(target)
    if anchor not in pos.board:
        raise SystemExit(f"target {args.target} ({target} on the crop) has no "
                         f"stone on the cropped board")
    if pos.board[anchor] != target_color:
        raise SystemExit(f"mode {mode} needs a {target_color} target; "
                         f"{args.target} is {pos.board[anchor]}")

    stones, libs = pos.group(anchor)
    moves = candidate_moves(pos, stones, args.radius)
    results = {mv: test_move(pos, mv, target, attacker, args.plies,
                             args.budget, mode, args.scope)
               for mv in moves}
    wins = sorted(mv for mv, (r, _) in results.items() if r == "win")
    unknown = sorted(mv for mv, (r, _) in results.items() if r == "unknown")

    print(f"{problem['title']} - {args.size}x{args.size} crop, {player} to move, "
          f"mode={mode}, scope={args.scope}, target={target} "
          f"({len(stones)} stones, {len(libs)} liberties), plies={args.plies}")
    for mv in moves:
        r, nodes = results[mv]
        print(f"  {mv:4s} {r:8s} ({nodes} nodes)")
    print(f"VERIFIED KEY MOVES: {wins if wins else 'none'}")
    if unknown:
        print(f"inconclusive (budget): {unknown}")

    if args.output:
        black, white = board_stones(pos)
        seed = {
            "board_size": args.size,
            "black_stones": black, "white_stones": white,
            "player_to_move": player,
            "objective": {
                "kind": "capture_within" if mode == "capture" else "avoid_capture_for",
                "attacker": attacker, "target": target, "plies": args.plies,
            },
            "verified_moves": wins,
            "inconclusive_moves": unknown,
            "verified_scope": args.scope,
            "candidate_radius": args.radius,
            "seed": {
                "book": problem["source"]["book"],
                "problem": problem["source"]["problem"],
                "title": problem["title"],
                "crop": f"top-left {args.size}x{args.size}",
                "swap_colors": args.swap_colors,
            },
        }
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(seed, f, indent=1)
        print(f"puzzle seed -> {args.output}", file=sys.stderr)
    return 0 if wins else 1


def scan_problems(problems, args):
    lo, hi = args.scan
    interesting = []
    for n in range(lo, min(hi, len(problems)) + 1):
        problem = problems[n - 1]
        try:
            pos, player = crop_problem(problem, args.size, args.swap_colors)
        except ValueError as e:
            print(f"problem {n}: skipped ({e})")
            continue
        mode = args.mode
        enemy = "W" if player == "B" else "B"
        attacker = enemy if mode == "survive" else player
        target_color = enemy if mode == "capture" else player
        for g in groups_of(pos, target_color):
            if len(g["stones"]) > args.max_stones:
                continue
            if mode == "survive":
                # only groups actually in danger are worth a defender puzzle
                q = pos.clone()
                q.to_move = attacker
                danger, _ = bounded_capture(q, g["anchor"], attacker,
                                            args.plies - 1, args.budget, args.scope)
                if danger is not True:
                    continue
            wins, unknown = [], []
            for mv in candidate_moves(pos, g["stones"], args.radius):
                r, _ = test_move(pos, mv, g["anchor"], attacker, args.plies,
                                 args.budget, mode, args.scope)
                if r == "win":
                    wins.append(mv)
                elif r == "unknown":
                    unknown.append(mv)
            if wins:
                tag = "UNIQUE" if len(wins) == 1 else f"{len(wins)} moves"
                print(f"problem {n} ({problem['title']}): target {g['anchor']} "
                      f"({len(g['stones'])} stones) -> {tag}: {sorted(wins)}"
                      + (f" [inconclusive: {unknown}]" if unknown else ""))
                interesting.append({
                    "problem": n, "title": problem["title"],
                    "target": g["anchor"], "stones": len(g["stones"]),
                    "wins": sorted(wins), "inconclusive": unknown,
                })
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(interesting, f, indent=1)
        print(f"{len(interesting)} seed candidates -> {args.output}",
              file=sys.stderr)
    if args.scope == "near":
        print("note: scan ran in near scope (fast screening) - confirm the "
              "chosen seed with a full-scope run (no --near) before authoring")
    return 0


def main():
    ap = argparse.ArgumentParser(
        description="Crop and mechanically verify tsumego puzzle seeds "
                    "(tasuki corpus). See references/tsumego-source-formats.md.")
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--json", help="problems JSON from parse_tasuki_tex.py")
    src.add_argument("--tex", help=".tex file path or raw GitHub URL")
    ap.add_argument("--problem", type=int, help="problem number (1-based)")
    ap.add_argument("--scan", metavar="A:B",
                    help="scan problem range A..B for seeds with a unique "
                         "verified key move")
    ap.add_argument("--mode", choices=["capture", "survive"], default="capture")
    ap.add_argument("--target",
                    help="anchor stone of the target group, original 19x19 "
                         "GTP coordinates (required without --scan)")
    ap.add_argument("--size", type=int, default=9, help="crop size, default 9")
    ap.add_argument("--radius", type=int, default=2,
                    help="first moves tested within this Chebyshev distance of "
                         "the target group, default 2")
    ap.add_argument("--plies", type=int, default=5,
                    help="objective horizon including the first move, default 5")
    ap.add_argument("--budget", type=int, default=200000,
                    help="node budget per search")
    ap.add_argument("--max-stones", type=int, default=6,
                    help="scan: skip target groups larger than this")
    ap.add_argument("--swap-colors", action="store_true",
                    help="swap colours and player (defender puzzles from "
                         "black-to-kill problems)")
    ap.add_argument("--near", action="store_true",
                    help="fast near-fight scope (default for --scan; not a "
                         "proof - confirm seeds in full scope)")
    ap.add_argument("--full", action="store_true",
                    help="sound all-legal-replies search "
                         "(puzzle_reading semantics; default for single "
                         "problems)")
    ap.add_argument("-o", "--output",
                    help="write the puzzle seed / scan results JSON here")
    args = ap.parse_args()
    if args.near and args.full:
        raise SystemExit("--near and --full are mutually exclusive")
    if args.near:
        args.scope = "near"
    elif args.full:
        args.scope = "full"
    elif args.scan:
        args.scope = "near"
    else:
        args.scope = "full"

    problems = load_problems(args.json or args.tex)

    if args.scan:
        a, b = (int(x) for x in args.scan.split(":"))
        args.scan = (a, b)
        sys.exit(scan_problems(problems, args))

    if not args.problem or not 1 <= args.problem <= len(problems):
        raise SystemExit(f"--problem N is required (1..{len(problems)})")
    sys.exit(verify_single(problems[args.problem - 1], args))


if __name__ == "__main__":
    main()
