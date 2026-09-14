#!/usr/bin/env python3
"""Check a lesson JSON for board-fact and coordinate mistakes before generating the HTML.

Usage: validate_lesson.py <parsed.json> <lesson.json>
Exit code 0 = clean, 1 = problems printed (one per line). Warnings do not fail.
"""
import json, re, sys

COLS = "ABCDEFGHJKLMNOPQRST"


def coord_ok(mv, size):
    if mv == 'pass':
        return True
    m = re.fullmatch(r'([A-T])(\d{1,2})', mv or '')
    if not m or m.group(1) not in COLS[:size]:
        return False
    return 1 <= int(m.group(2)) <= size


def xy(mv, size):
    return COLS.index(mv[0]), size - int(mv[1:])


def replay(moves, upto, size):
    """Board dict after `upto` moves of the parsed compact list (captures handled)."""
    board = {}
    def neighbours(p):
        x, y = p
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < size and 0 <= ny < size:
                yield (nx, ny)
    def group(p):
        col = board[p]; seen = {p}; stack = [p]; libs = 0
        while stack:
            q = stack.pop()
            for n in neighbours(q):
                if n not in board:
                    libs += 1
                elif board[n] == col and n not in seen:
                    seen.add(n); stack.append(n)
        return seen, libs
    for m in moves[:upto]:
        if m['move'] == 'pass':
            continue
        p = xy(m['move'], size); board[p] = m['color']
        for n in list(neighbours(p)):
            if n in board and board[n] != m['color']:
                g, libs = group(n)
                if libs == 0:
                    for q in g:
                        board.pop(q, None)
    return board


def main():
    parsed = json.load(open(sys.argv[1])); lesson = json.load(open(sys.argv[2]))
    size = parsed['game_info'].get('board_size', 19)
    moves = parsed['moves']
    problems, warnings = [], []
    student = parsed['game_info'].get('student', 'B')

    if not lesson.get('game_arc') or not lesson['game_arc'].get('phases'):
        problems.append("game_arc missing or has no phases")
    for i, l in enumerate(lesson.get('lessons', [])):
        tag = f"lesson {i+1} (move {l.get('move_number')})"
        n = l.get('move_number')
        if not isinstance(n, int) or n < 1 or n > len(moves):
            problems.append(f"{tag}: move_number outside the game (1..{len(moves)})"); continue
        real = moves[n - 1]
        if l.get('played_move') != real['move']:
            problems.append(f"{tag}: played_move {l.get('played_move')} but the game record has {real['move']}")
        if l.get('player_color', real['color']) != real['color']:
            problems.append(f"{tag}: player_color {l.get('player_color')} but move {n} was played by {real['color']}")
        if real['color'] != student:
            warnings.append(f"{tag}: this move was played by the opponent, not the student")
        if not l.get('story'):
            problems.append(f"{tag}: story missing")
        board = replay(moves, n - 1, size)
        for key in ('preferred_move',):
            mv = l.get(key)
            if mv and mv != 'pass':
                if not coord_ok(mv, size):
                    problems.append(f"{tag}: {key} {mv} is not a valid coordinate")
                elif xy(mv, size) in board:
                    problems.append(f"{tag}: {key} {mv} is occupied before move {n}")
        for a in l.get('alternatives', []):
            mv = a.get('move')
            if not coord_ok(mv, size) or (mv != 'pass' and xy(mv, size) in board):
                problems.append(f"{tag}: alternative {mv} invalid or occupied")
        for seq_name in ('refutation', 'better_line'):
            for mv in l.get(seq_name, []):
                if not coord_ok(mv, size):
                    problems.append(f"{tag}: {seq_name} contains invalid coordinate {mv}")
        cand = next((c for c in parsed['teaching']['candidates'] if c['move_number'] == n), None)
        if cand and l.get('refutation') and cand.get('refutation') and l['refutation'][:2] != cand['refutation'][:2]:
            warnings.append(f"{tag}: refutation differs from the report's ({' '.join(cand['refutation'][:4])})")

    for i, p in enumerate(lesson.get('puzzles', [])):
        tag = f"puzzle {i+1}"
        bs = p.get('board_size', 9)
        stones = {}
        for col, key in (('B', 'black_stones'), ('W', 'white_stones')):
            for mv in p.get(key, []):
                if not coord_ok(mv, bs):
                    problems.append(f"{tag}: {key} has invalid coordinate {mv}"); continue
                if xy(mv, bs) in stones:
                    problems.append(f"{tag}: two stones on {mv}")
                stones[xy(mv, bs)] = col
        olm = p.get('opponent_last_move')
        if olm and (not coord_ok(olm, bs) or xy(olm, bs) not in stones):
            problems.append(f"{tag}: opponent_last_move {olm} is not a stone on the board")
        correct = p.get('correct_moves', [])
        if not correct:
            problems.append(f"{tag}: no correct_moves")
        for mv in correct:
            if not coord_ok(mv, bs) or xy(mv, bs) in stones:
                problems.append(f"{tag}: correct move {mv} invalid or occupied")
        for w in p.get('wrong_moves', []):
            mv = w.get('move')
            if not coord_ok(mv, bs) or xy(mv, bs) in stones:
                problems.append(f"{tag}: wrong move {mv} invalid or occupied")
            if mv in correct:
                problems.append(f"{tag}: wrong move {mv} is also listed as correct")
            occupied = set(stones) | ({xy(mv, bs)} if coord_ok(mv, bs) and mv != 'pass' else set())
            for r in w.get('refutation', []):
                if not coord_ok(r, bs) or xy(r, bs) in occupied:
                    problems.append(f"{tag}: refutation move {r} after {mv} is invalid or occupied")
                else:
                    occupied.add(xy(r, bs))
            if not w.get('explanation'):
                problems.append(f"{tag}: wrong move {mv} has no explanation")
        # a puzzle must not be the game position
        for c in parsed['teaching']['candidates']:
            if set(p.get('black_stones', [])) == set(c.get('black_stones', [])) and set(p.get('white_stones', [])) == set(c.get('white_stones', [])) and stones:
                problems.append(f"{tag}: identical to the game position before move {c['move_number']}")
    for w in warnings:
        print("warning:", w)
    for p in problems:
        print("PROBLEM:", p)
    print(f"{len(problems)} problem(s), {len(warnings)} warning(s)")
    sys.exit(1 if problems else 0)


if __name__ == '__main__':
    main()
