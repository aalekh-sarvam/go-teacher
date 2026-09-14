"""Join teacher prose to immutable report evidence. No model transcription of engine data."""
from copy import deepcopy
from go_rules import sequence_position

PANELS = ('position', 'played', 'best', 'alternatives', 'local', 'what_if')
FORBIDDEN = ('point_loss','played_move','preferred_move','player_color','refutation','better_line','alternatives','evidence','sequences','tree','ownership_plan','score_before','score_after','winrate_before','winrate_after')

def hydrate(parsed, authored):
    result = deepcopy(authored)
    schema = result.get('schema_version',1)
    if schema not in (1,2): raise ValueError(f'unsupported lesson schema {schema}')
    if schema == 2 and parsed.get('report_format') != 5: raise ValueError('lesson schema 2 requires report format 5')
    gi = parsed['game_info']
    if gi.get('board_size_y', gi.get('board_size')) != gi.get('board_size'):
        raise ValueError('The lesson renderer currently supports square boards only.')
    result.setdefault('players', {'black':gi.get('black_player') or 'Black','white':gi.get('white_player') or 'White'})
    result.setdefault('result', gi.get('result') or '')
    candidates = {c['move_number']:c for c in parsed['teaching']['candidates']}
    for lesson in result.get('lessons',[]):
        n = lesson.get('move_number')
        if schema == 2:
            forbidden = [k for k in FORBIDDEN if k in lesson]
            if forbidden: raise ValueError(f'move {n}: schema 2 writes prose only; remove {forbidden}')
            if n not in candidates: raise ValueError(f'move {n}: no teaching evidence for that reference')
        if parsed.get('report_format') != 5 or n not in candidates: continue
        c = candidates[n]; evidence = deepcopy(c.get('evidence',{}))
        for sequence in evidence.get('sequences',[]): sequence_position(parsed,sequence)
        for trial in (evidence.get('local_reading') or {}).get('trials',[]):
            if trial.get('sequence'): sequence_position(parsed,trial['sequence'])
        texts = lesson.get('panels',{})
        if not isinstance(texts,dict) or any(k not in PANELS for k in texts): raise ValueError(f'move {n}: unknown panel')
        if any(not isinstance(v,str) for v in texts.values()): raise ValueError(f'move {n}: panel text must be a string')
        prose = lesson.get('alternative_explanations',{})
        detail = parsed.get('move_details',{}).get(str(n),{})
        valid_moves={a['move'] for a in detail.get('candidates',[])} | {a['move'] for a in evidence.get('tree',[])}
        if any(m not in valid_moves for m in prose): raise ValueError(f'move {n}: alternative text refers to an unsearched move')
        lesson.update(played_move=c['played_move'],preferred_move=c.get('preferred_move'),player_color=c['player'],point_loss=c['point_loss'],theme=lesson.get('theme',c['theme']),
            refutation=c.get('refutation',[]),better_line=c.get('better_line',[]),_evidence=evidence,
            explanation=texts.get('played',lesson.get('explanation','')),variation_explanation=texts.get('best',lesson.get('variation_explanation','')),
            refutation_explanation=texts.get('played',lesson.get('refutation_explanation','')),
            alternatives=[dict(a,explanation=prose.get(a['move'],'')) for a in detail.get('candidates',[])])
        lesson['_difficulty'] = c.get('difficulty',{})
    return result


def brief(parsed):
    """A model-facing reading view. Keep the original parsed file for validation/generation."""
    out = deepcopy(parsed)
    out.pop('moves',None);out.pop('move_details',None)
    for c in out['teaching']['candidates']:
        e = c.get('evidence',{})
        e['sequences'] = [{k:v for k,v in s.items() if k not in ('probabilities','seed')} for s in e.get('sequences',[])]
        for s in e['sequences']:
            s['total_plies'] = len(s['moves']);s['moves'] = s['moves'][:6];s['preview_only'] = True
        # Read the exact selected line from parsed.json before narrating numbered stones.
        for key in ('black_stones','white_stones'): c.pop(key,None)
        if 'tree' in e: e['tree']=[{k:v for k,v in t.items() if k!='children'} for t in e['tree']]
        lr=e.get('local_reading') or {}
        lr.pop('allowed_moves',None)
        for trial in lr.get('trials',[]): trial.pop('sequence',None)
    out['reading_view_only'] = True
    return out
