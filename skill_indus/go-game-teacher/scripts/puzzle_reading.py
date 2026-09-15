"""Bounded capture proof with all legal board replies. No life/death or score oracle.
An attacker seeks to capture the original target group; the defender seeks to prevent it
for the stated number of plies. None means the work budget was exhausted, not a result.
"""
from go_rules import COLS


def capture_within(position, target, attacker, plies, node_budget=50000):
    anchor = position.xy(target)
    if anchor not in position.board: return True, 0
    if position.board[anchor] == attacker: raise ValueError('capture target belongs to attacker')
    count = 0
    def search(p, depth):
        nonlocal count
        if anchor not in p.board or p.board[anchor] == attacker: return True
        if depth <= 0: return False
        if count >= node_budget: return None
        count += 1
        # Search adjacent liberties first for speed, but include ALL legal board moves and pass.
        _, libs = p.group(anchor)
        points = sorted(libs) + [xy for xy in ((x,y) for y in range(p.sy) for x in range(p.sx)) if xy not in p.board and xy not in libs]
        options = [COLS[x]+str(p.sy-y) for x,y in points] + ['pass']
        attacking = p.to_move == attacker
        unknown = False
        for mv in options:
            q=p.clone()
            try: q.play(mv)
            except ValueError: continue
            r=search(q,depth-1)
            if r is None: unknown=True
            elif r == attacking: return r
        return None if unknown else not attacking
    return search(position,plies), count


def check_capture_choice(puzzle, move, goal, budget=50000):
    from go_rules import Position
    p=Position(puzzle.get('board_size',9),black=puzzle.get('black_stones',[]),white=puzzle.get('white_stones',[]),to_move=puzzle.get('player_to_move','B'),rules=puzzle.get('rules','japanese'))
    anchor=p.xy(goal['target']); attacker=goal['attacker']; depth=goal['plies']
    if attacker not in ('B','W') or anchor not in p.board or p.board[anchor]==attacker:
        raise ValueError('objective must target an existing opponent group')
    if not isinstance(depth,int) or depth<1: raise ValueError('objective needs a positive ply horizon including the first choice')
    expected_player = attacker if goal['kind']=='capture_within' else ('W' if attacker=='B' else 'B')
    if p.to_move != expected_player: raise ValueError('objective and puzzle player disagree')
    p.play(move)
    if anchor not in p.board or p.board[anchor] == attacker: captured, nodes=True,0
    else: captured,nodes=capture_within(p,goal['target'],attacker,depth-1,budget)
    if captured is None: return None,nodes
    return (captured if goal['kind']=='capture_within' else not captured),nodes
