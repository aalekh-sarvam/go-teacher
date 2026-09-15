"""Check explicit draft claims and puzzle board assertions; prose semantics still need review."""
from teaching_facts import build_facts, pointer, expand_facts, group_fact
from go_rules import Position
from puzzle_reading import check_capture_choice


def check_grounding(parsed, authored):
    errors=[]
    if authored.get('grounding_version') != 1: return errors
    facts=build_facts(parsed); draft=expand_facts(authored,facts)
    checks=draft.get('fact_checks',[])
    covered=set()
    for i,c in enumerate(checks):
        try:
            actual=pointer(facts,c['fact'])
            if actual != c['expected']: errors.append(f'fact check {i+1}: {c["fact"]} is {actual!r}, not {c["expected"]!r}')
            text=pointer(draft,c['text_path'])
            if not isinstance(text,str) or not c.get('quote') or c['quote'] not in text:
                errors.append(f'fact check {i+1}: quote must occur in the referenced explanation')
            covered.add(c['text_path'].split('/')[2] if c['text_path'].startswith('/lessons/') else 'overview')
        except (KeyError,IndexError,ValueError,TypeError) as e: errors.append(f'fact check {i+1}: invalid reference ({e})')
    for i in range(len(draft.get('lessons',[]))):
        if str(i) not in covered: errors.append(f'lesson {i+1}: add fact_checks linking prose to the facts used')
    for i,gm in enumerate(draft.get('good_moves',[])):
        n=gm.get('move_number'); actual=facts['actual_moves'].get(str(n))
        if not actual or actual['color']!=facts['student'] or actual['move']!=gm.get('move'):
            errors.append(f'praise {i+1}: must reference the student\'s actual move')
        if gm.get('rating') and not gm.get('rating_source'): errors.append(f'praise {i+1}: omit unsupported rank rating')
    return errors


def check_puzzle(puzzle):
    errors=[]
    source=puzzle.get('source',{})
    if not source.get('example_locator'):
        errors.append('source.example_locator must identify the specific external diagram/problem, not just a course')
    if not source.get('position'):
        errors.append('source.position must record the external example used for the transformation')
    else:
        from validate_lesson import canonical
        sp=source['position']; original=Position(sp.get('board_size',9),black=sp.get('black_stones',[]),white=sp.get('white_stones',[]),to_move=sp.get('player_to_move','B'))
        changed=Position(puzzle.get('board_size',9),black=puzzle.get('black_stones',[]),white=puzzle.get('white_stones',[]),to_move=puzzle.get('player_to_move','B'))
        if not original.board or canonical(original.board,original.sx,original.sy)==canonical(changed.board,changed.sx,changed.sy):
            errors.append('transformation must change the reading, beyond symmetry/colour/translation')
    goal=puzzle.get('objective',{})
    if goal.get('kind') not in ('capture_within','avoid_capture_for','compare_plans') or not goal.get('description'):
        errors.append('state an objective: capture_within, avoid_capture_for, or compare_plans')
    review=puzzle.get('solution_review',{})
    if review.get('method') not in ('exhaustive_capture','engine','external_solution','reading'):
        errors.append('solution_review.method must name how the new board was verified')
    if not review.get('non_obvious_reason'): errors.append('explain the plausible decoy and extra reading step')
    if not review.get('limitations'): errors.append('state verification limits, including any unproved life/death or optimality')
    if goal.get('unique_best'):
        errors.append('finite search cannot prove unique whole-board optimality; phrase the objective as a searched preference or a bounded tactical goal')
    if review.get('method')=='engine':
        try:
            from verify_puzzle_engine import check_artifact
            errors.extend(check_artifact(puzzle,review['engine_artifact']))
        except (OSError,KeyError,ValueError,TypeError) as e: errors.append(f'engine artifact: {e}')
    correct=puzzle.get('correct_moves',[]); wrong=[w['move'] for w in puzzle.get('wrong_moves',[])]
    defended=set()
    for line in review.get('strongest_defences',[]):
        moves=line.get('moves',[])
        if not moves or moves[0] not in correct+wrong or not line.get('why_strongest') or not line.get('conclusion'):
            errors.append('each strongest defence needs a tested first choice, full moves, why_strongest and conclusion'); continue
        p=Position(puzzle.get('board_size',9),black=puzzle.get('black_stones',[]),white=puzzle.get('white_stones',[]),to_move=puzzle.get('player_to_move','B'),rules=puzzle.get('rules','japanese'))
        for mv in moves:p.play(mv)
        shown = puzzle.get('correct_lines',{}).get(moves[0]) if moves[0] in correct else next(([w['move']]+w.get('refutation',[]) for w in puzzle.get('wrong_moves',[]) if w['move']==moves[0]),None)
        if shown != moves:
            errors.append(f'{moves[0]}: displayed continuation must match the reviewed strongest-defence line')
        defended.add(moves[0])
    if set(correct+wrong)-defended: errors.append('read strongest defences for every offered correct and wrong choice')
    checks=puzzle.get('board_checks',[])
    if not checks: errors.append('add board_checks for the tactical claims (liberties, presence or stones)')
    for c in checks:
        p=Position(puzzle.get('board_size',9),black=puzzle.get('black_stones',[]),white=puzzle.get('white_stones',[]),to_move=puzzle.get('player_to_move','B'),rules=puzzle.get('rules','japanese'))
        for mv in c.get('after',[]):p.play(mv)
        fact=group_fact(p,c['anchor'])
        if not c.get('expected'): errors.append('board check needs expected facts')
        for key,value in c.get('expected',{}).items():
            if key not in fact or fact[key]!=value: errors.append(f'board check {c["anchor"]} after {c.get("after",[])}: {key} is {fact.get(key)!r}, not {value!r}')
    if goal.get('kind') in ('capture_within','avoid_capture_for'):
        for mv in correct+wrong:
            result,_=check_capture_choice(puzzle,mv,goal)
            if result is None: errors.append(f'{mv}: capture reading inconclusive at the work limit; use a verified simpler exercise')
            elif result != (mv in correct): errors.append(f'{mv}: strongest-defence reading contradicts its answer label for the stated objective')
    return errors
