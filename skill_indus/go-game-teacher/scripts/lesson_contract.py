"""Join teacher prose to immutable report evidence. No model transcription of engine data."""
import re
from copy import deepcopy
from go_rules import sequence_position
from teaching_facts import build_facts, expand_facts

PANELS = ('position', 'played', 'best', 'alternatives', 'local', 'what_if')
FORBIDDEN = ('point_loss','played_move','preferred_move','player_color','refutation','better_line','alternatives','evidence','sequences','tree','ownership_plan','score_before','score_after','winrate_before','winrate_after')
_RANGE_TAIL = re.compile(r'\d\s*[+\-\u2013]\s*$')

def parse_move_range(text):
    """(start, end) move numbers from a phase move_range string, or None.
    Accepts '12-45', '45+', '34', en-dashes and stray words around the numbers.
    An open-ended tail ('45+' / '50-') gives end=None: 'to the end of the game'."""
    if text is None:
        return None
    s = str(text)
    numbers = re.findall(r'\d+', s)
    if not numbers:
        return None
    start = int(numbers[0])
    end = int(numbers[1]) if len(numbers) > 1 else start
    if end < start:
        start, end = end, start
    if _RANGE_TAIL.search(s):
        end = None
    return start, end

def hydrate(parsed, authored):
    if authored.get('grounding_version') not in (None,1): raise ValueError('unsupported grounding_version')
    result = deepcopy(authored)
    if authored.get('grounding_version') == 1:
        facts = build_facts(parsed)
        result = expand_facts(result, facts)
        if result.get('progress'):
            result['progress']['rows'] = facts['current']['progress_rows']
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
        sequence_texts = lesson.get('sequence_explanations',{})
        valid_ids = {s['id'] for s in evidence.get('sequences',[])} | {t['sequence']['id'] for t in (evidence.get('local_reading') or {}).get('trials',[]) if t.get('sequence')}
        if not isinstance(sequence_texts,dict) or any(k not in valid_ids or not isinstance(v,str) for k,v in sequence_texts.items()):
            raise ValueError(f'move {n}: sequence explanation must reference an available sequence ID')
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
    derive_context_anchors(result, parsed)
    return result

def derive_context_anchors(result, parsed):
    """Fill the anchor moves the context board jumps to. Authored anchor_move
    wins; otherwise the end of the phase's move_range; the overview defaults
    to the final position. Everything is clamped to the game so the board can
    always render the position it points to."""
    last = len(parsed.get('moves') or [])
    def settle(anchor):
        if anchor is None:
            return None
        if isinstance(anchor, bool) or not isinstance(anchor, int):
            raise ValueError(f'anchor move must be an integer, not {anchor!r}')
        return max(1, min(anchor, last)) if last else anchor
    overview = result.get('overview_anchor_move')
    result['overview_anchor_move'] = settle(overview if overview is not None else (last or None))
    arc = result.get('game_arc')
    if isinstance(arc, dict):
        for ph in arc.get('phases') or []:
            if not isinstance(ph, dict):
                continue
            anchor = ph.get('anchor_move')
            if anchor is None:
                span = parse_move_range(ph.get('move_range'))
                if span is None:
                    continue  # no resolvable range: the phase card renders without a jump
                anchor = span[1] if span[1] is not None else last
            ph['anchor_move'] = settle(anchor)


def brief(parsed):
    """A model-facing reading view. Keep the original parsed file for validation/generation."""
    out = deepcopy(parsed)
    # Keep the compact game timeline: it is the context for phase/lead observations.
    # Full candidate tables still live in parsed.json alongside untruncated sequences.
    out.pop('move_details',None)
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
