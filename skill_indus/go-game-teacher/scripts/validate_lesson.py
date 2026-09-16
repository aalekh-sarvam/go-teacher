#!/usr/bin/env python3
"""Validate evidence references and replay legality. This checks board mechanics, not Go truth.
Usage: validate_lesson.py parsed.json lesson.json
"""
import json
import re
import sys
from urllib.parse import urlparse
from parse_review import coverage_summary
from go_rules import Position, sequence_position
from lesson_contract import hydrate, PANELS
from check_grounding import check_grounding, check_puzzle


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


# Prose fields of a lesson that may carry coverage or sequence claims.
PROSE_FIELDS = ('title','story','principle','level_framing','explanation','variation_explanation','refutation_explanation')
_VERIFIED = re.compile(r'\b(?:deeply\s+verified|verified|deep(?:er)?\s+(?:evaluation|search|analysis|re-?analysis)|deeply\s+(?:evaluated|analys[ez]d|searched))\b', re.I)
_NEGATED = re.compile(r'\b(?:no|not|never|without|cannot|could not|was not|were not|isn\'t|wasn\'t)\W*$', re.I)
_INVESTIGATION_PROSE = re.compile(r'\b(?:human\s+(?:reply|replies|continuation|sequence|line|sample|profile\s+(?:line|sequence|sample))s?|sampled\s+(?:future|line|continuation|game)s?|restricted\s+(?:reading|search)|local\s+reading|what[- ]if)\b', re.I)
NO_INVESTIGATION_STATUSES = ('not_selected','disabled','unavailable')


def _prose(lesson):
    out=[(k,lesson.get(k)) for k in PROSE_FIELDS]
    panels=lesson.get('panels')
    if isinstance(panels,dict): out+=[(f'panels.{k}',v) for k,v in panels.items()]
    return [(k,v) for k,v in out if isinstance(v,str) and v.strip()]


def _claims(pattern, text):
    for m in pattern.finditer(text):
        if not _NEGATED.search(text[max(0,m.start()-24):m.start()]): yield m[0]


def check_coverage_claims(parsed, authored):
    """Coverage contradictions: a 'verified' claim needs evaluation_coverage.status == 'complete'; prose about
    human/local/what-if sequences needs investigations that exist. Lessons without coverage fields stay valid
    (unknown coverage) unless they claim verification. Returns (errors, warnings)."""
    errors,warnings=[],[]
    cov={c['move_number']:c for c in coverage_summary(parsed)}
    lessons=[l for l in authored.get('lessons',[]) if isinstance(l,dict)]
    for i,l in enumerate(lessons):
        tag=f'lesson {i+1}';n=l.get('move_number');c=cov.get(n)
        if c is None: continue
        for field,text in _prose(l):
            if c['evaluation_status']!='complete':
                for claim in _claims(_VERIFIED,text):
                    errors.append(f'{tag}: {field} says "{claim}" but evaluation_coverage.status for move {n} is {c["evaluation_status"]!r}; only "complete" supports that claim')
                    break
            if c['investigation_status'] in NO_INVESTIGATION_STATUSES:
                if field in ('panels.local','panels.what_if'):
                    warnings.append(f'{tag}: {field} is written but investigation_coverage for move {n} is {c["investigation_status"]!r}: no local/what-if sequences exist for it; omit the panel')
                else:
                    for claim in _claims(_INVESTIGATION_PROSE,text):
                        warnings.append(f'{tag}: {field} mentions "{claim}" but investigation_coverage for move {n} is {c["investigation_status"]!r}: those sequences do not exist; use refutation, better line, policies and chain')
                        break
    if lessons:
        first=cov.get(lessons[0].get('move_number'))
        if first and first['evaluation_status']=='incomplete' and any(c['evaluation_status']=='complete' for c in cov.values()):
            warnings.append(f'lesson 1: primary lesson uses move {first["move_number"]} whose deep evaluation is incomplete while other candidates are complete; prefer a complete candidate or keep the uncertainty visible')
    return errors,warnings


def validate(parsed, authored):
    errors, warnings = [], []
    if parsed.get('reading_view_only'): return ['Use the complete parsed.json, not the brief reading view.'], []
    try: lesson=hydrate(parsed,authored)
    except (ValueError,KeyError,IndexError,TypeError) as e: return [str(e)], []
    schema=authored.get('schema_version',1)
    try: errors.extend(check_grounding(parsed, authored))
    except (KeyError,IndexError,ValueError,TypeError) as e: errors.append(f'grounding: {e}')
    if schema==2 and authored.get('grounding_version')!=1:
        warnings.append('Legacy draft: no fact audit or stronger puzzle checks. New lessons must set grounding_version: 1.')
    cov_errors,cov_warnings=check_coverage_claims(parsed, authored); errors.extend(cov_errors); warnings.extend(cov_warnings)
    gi=parsed['game_info'];moves=parsed['moves'];student=gi.get('student','B')
    if not lesson.get('game_arc',{}).get('phases'): errors.append('game_arc must contain phases')
    if not lesson.get('lessons'): errors.append('at least one teaching lesson is required')
    # Context-board anchors: derived/clamped by the contract layer; authored
    # values outside the game still get a warning so the author can fix them.
    for i, ph in enumerate((authored.get('game_arc') or {}).get('phases') or []):
        a = ph.get('anchor_move') if isinstance(ph, dict) else None
        if isinstance(a, int) and not isinstance(a, bool) and not 1 <= a <= len(moves):
            warnings.append(f'game_arc phase {i+1}: anchor_move {a} is outside the game and will be clamped')
    a = authored.get('overview_anchor_move')
    if isinstance(a, int) and not isinstance(a, bool) and not 1 <= a <= len(moves):
        warnings.append(f'overview_anchor_move {a} is outside the game and will be clamped')
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
        authored_theme=l.get('theme');candidate_theme=references.get(n,{}).get('theme')   # hydrate defaults theme to the candidate's
        if authored_theme and candidate_theme and authored_theme!=candidate_theme:
            warnings.append(f'{tag}: theme {authored_theme!r} differs from the report\'s theme {candidate_theme!r} for move {n}; keep the report theme unless the prose states the reason')
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
        # Keys the generator dereferences without a default: fail here, not while rendering.
        for field in ('title','concept_label','hint','explanation'):
            if not isinstance(puzzle.get(field),str) or not puzzle[field].strip(): errors.append(f'{tag}: {field} missing')
        if puzzle.get('player_to_move') not in ('B','W'): errors.append(f'{tag}: player_to_move must be "B" or "W"')
        for k,wrong in enumerate(puzzle.get('wrong_moves',[])):
            if not isinstance(wrong,dict) or not isinstance(wrong.get('move'),str): errors.append(f'{tag}: wrong move {k+1} needs a move')
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
            if authored.get('grounding_version') == 1:
                errors.extend(f'{tag}: {e}' for e in check_puzzle(puzzle))
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
    praise_rows={p.get('move_number',p.get('number')):p for p in parsed.get('teaching',{}).get('praise',[]) or []}
    for i,gm in enumerate(lesson.get('good_moves',[])):
        tag=f'good move {i+1}'
        if not isinstance(gm,dict): errors.append(f'{tag}: must be an object');continue
        n=gm.get('move_number')
        if not isinstance(n,int) or isinstance(n,bool) or not 1<=n<=len(moves): errors.append(f'{tag}: move_number outside game');continue
        real=moves[n-1]
        if real['color']!=student: errors.append(f'{tag}: move {n} was played by the opponent, not the student')
        if gm.get('move')!=real['move']: errors.append(f'{tag}: move {gm.get("move")!r} is not the recorded move {real["move"]} at {n}')
        if not isinstance(gm.get('explanation'),str) or not gm['explanation'].strip(): errors.append(f'{tag}: explanation missing')
        if gm.get('rating') and not gm.get('rating_source'): errors.append(f'{tag}: rating requires rating_source (copy both from teaching.praise)')
        if gm.get('rating') and n in praise_rows and praise_rows[n].get('rating') and gm['rating']!=praise_rows[n]['rating']:
            errors.append(f'{tag}: rating {gm["rating"]!r} differs from the report\'s {praise_rows[n]["rating"]!r}')
    for i,concept in enumerate(lesson.get('concepts_learned',[])):
        tag=f'concept {i+1}'
        if not isinstance(concept,dict): errors.append(f'{tag}: must be an object');continue
        for field in ('term','description'):
            if not isinstance(concept.get(field),str) or not concept[field].strip(): errors.append(f'{tag}: {field} missing')
        for k,res in enumerate(concept.get('resources') or []):
            if not isinstance(res,dict) or not isinstance(res.get('title'),str) or not isinstance(res.get('url'),str) or not res['url'].strip():
                errors.append(f'{tag}: resource {k+1} needs title and url')
            elif urlparse(res['url']).scheme not in ('http','https'): errors.append(f'{tag}: resource {k+1} url must be http(s)')
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
