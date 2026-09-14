#!/usr/bin/env python3
"""Validate evidence references and replay legality. This checks board mechanics, not Go truth.
Usage: validate_lesson.py parsed.json lesson.json
"""
import json
import sys
from urllib.parse import urlparse
from go_rules import Position, sequence_position
from lesson_contract import hydrate, PANELS


def coord_ok(mv, size):
    try: Position(size).xy(mv); return True
    except (ValueError,TypeError): return False


def canonical(board, sx, sy):
    """Translation/rotation/reflection/colour-swap invariant shape, for puzzle reuse detection."""
    forms = []
    for swap_axes in (False,True):
        for flip_x in (False,True):
            for flip_y in (False,True):
                for invert in (False,True):
                    points=[]
                    for (x,y),c in board.items():
                        if swap_axes: x,y=y,x
                        if flip_x: x=-x
                        if flip_y: y=-y
                        if invert: c='W' if c=='B' else 'B'
                        points.append((x,y,c))
                    if not points: continue
                    x0=min(p[0] for p in points);y0=min(p[1] for p in points)
                    forms.append(tuple(sorted((x-x0,y-y0,c) for x,y,c in points)))
    return min(forms) if forms else ()


def validate(parsed, authored):
    errors, warnings = [], []
    if parsed.get('reading_view_only'): return ['Use the complete parsed.json, not the brief reading view.'], []
    try: lesson=hydrate(parsed,authored)
    except (ValueError,KeyError,TypeError) as e: return [str(e)], []
    schema=authored.get('schema_version',1)
    gi=parsed['game_info'];moves=parsed['moves'];student=gi.get('student','B')
    if not lesson.get('game_arc',{}).get('phases'): errors.append('game_arc must contain phases')
    if not lesson.get('lessons'): errors.append('at least one teaching lesson is required')
    game_shapes=set()
    try:
        p=Position.from_parsed(parsed,0)
        game_shapes.add(canonical(p.board,p.sx,p.sy))
        for m in moves:
            p.to_move=m['color'];p.play(m['move']);game_shapes.add(canonical(p.board,p.sx,p.sy))
    except (KeyError,TypeError,ValueError) as e: errors.append(f'game replay: {e}')
    references={c['move_number']:c for c in parsed['teaching']['candidates']}
    for i,l in enumerate(lesson.get('lessons',[])):
        tag=f'lesson {i+1}';n=l.get('move_number')
        if not isinstance(n,int) or not 1<=n<=len(moves): errors.append(f'{tag}: move_number outside game');continue
        real=moves[n-1]
        if l.get('played_move')!=real['move']: errors.append(f'{tag}: played move differs from record')
        if l.get('player_color',real['color'])!=real['color']: errors.append(f'{tag}: wrong player colour')
        if real['color']!=student: errors.append(f'{tag}: selected move belongs to opponent')
        for field in ('title','concept_label','story','principle'):
            if not isinstance(l.get(field),str) or not l[field].strip(): errors.append(f'{tag}: {field} missing')
        if schema==2:
            panels=l.get('panels',{})
            for key in ('played','best'):
                if not panels.get(key): errors.append(f'{tag}: {key} panel explanation missing')
        try:
            base=Position.from_parsed(parsed,n-1)
            for a in l.get('alternatives',[]): base.clone().play(a['move'])
            if l.get('preferred_move'): base.clone().play(l['preferred_move'])
            # Format 5 always uses structured sequences (with their original histories).
            if parsed.get('report_format')==5:
                for seq in l.get('_evidence',{}).get('sequences',[]): sequence_position(parsed,seq)
            else:
                for name,prefix in [('better_line',[]),('refutation',[l['played_move']])]:
                    p=base.clone()
                    for mv in prefix+l.get(name,[]): p.play(mv)
        except (KeyError,ValueError,TypeError) as e: errors.append(f'{tag}: {e}')

    for i,puzzle in enumerate(lesson.get('puzzles',[])):
        tag=f'puzzle {i+1}';bs=puzzle.get('board_size',9);side=puzzle.get('player_to_move','B')
        try:
            p=Position(bs,black=puzzle.get('black_stones',[]),white=puzzle.get('white_stones',[]),to_move=side,rules=puzzle.get('rules','japanese'))
            if p.board and canonical(p.board,p.sx,p.sy) in game_shapes: errors.append(f'{tag}: reuses a game position or its symmetry/colour swap')
            for stone in p.board:
                if not p.group(stone)[1]: errors.append(f'{tag}: setup contains a group without liberties');break
            last=puzzle.get('opponent_last_move')
            if last and (p.xy(last) not in p.board or p.board[p.xy(last)]==side): errors.append(f'{tag}: opponent_last_move must be an opponent stone')
            answers=puzzle.get('correct_moves',[])
            if not answers: errors.append(f'{tag}: correct_moves empty')
            for answer in answers:
                p.clone().play(answer)
                if schema==2:
                    line=puzzle.get('correct_lines',{}).get(answer)
                    if not line or line[0]!=answer: errors.append(f'{tag}: provide a correct_lines sequence starting with {answer}')
                    else:
                        q=p.clone()
                        for mv in line:q.play(mv)
            for wrong in puzzle.get('wrong_moves',[]):
                mv=wrong['move'];q=p.clone();q.play(mv)
                if mv in answers: errors.append(f'{tag}: wrong move {mv} is also correct')
                for reply in wrong.get('refutation',[]): q.play(reply)
                if not wrong.get('explanation'): errors.append(f'{tag}: wrong move {mv} has no explanation')
                if schema==2:
                    if not wrong.get('refutation'): errors.append(f'{tag}: wrong move {mv} needs a refutation')
                    if wrong.get('loss_vs_best') is not None and not wrong.get('evaluation_source'):
                        errors.append(f'{tag}: numerical loss for {mv} needs evaluation_source for this puzzle')
            if schema==2:
                source=puzzle.get('source',{});url=urlparse(source.get('url',''))
                if url.scheme not in ('https','http') or not url.netloc: errors.append(f'{tag}: external source URL required')
                for key in ('transformation','verification','transfer_explanation'):
                    if not puzzle.get(key): errors.append(f'{tag}: {key} required')
                n=puzzle.get('lesson_move_number')
                if n not in references: errors.append(f'{tag}: unknown lesson_move_number')
                focus=puzzle.get('evidence_focus')
                allowed={'pass_comparison','move_values','initiative','ownership_plan','local_reading','tree','human_refutations','rollout_comparisons','difficulty','missed_opportunities','rank_fit'}
                if focus not in allowed: errors.append(f'{tag}: evidence_focus must name a supported teaching signal')
                if not puzzle.get('wrong_moves'): errors.append(f'{tag}: supply tempting wrong choices')
        except (KeyError,ValueError,TypeError) as e: errors.append(f'{tag}: {e}')
    if not lesson.get('puzzles'): warnings.append('No practice quizzes included')
    return errors,warnings


def main():
    with open(sys.argv[1],encoding='utf-8') as f: parsed=json.load(f)
    with open(sys.argv[2],encoding='utf-8') as f: lesson=json.load(f)
    errors,warnings=validate(parsed,lesson)
    for w in warnings: print('warning:',w)
    for e in errors: print('PROBLEM:',e)
    print(f'{len(errors)} problem(s), {len(warnings)} warning(s)')
    sys.exit(bool(errors))

if __name__=='__main__':main()
