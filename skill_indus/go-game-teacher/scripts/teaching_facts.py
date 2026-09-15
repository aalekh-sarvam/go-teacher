#!/usr/bin/env python3
"""Derive a reading aid from the report, without changing or enlarging the user's upload.
Usage: teaching_facts.py parsed.json facts.json [--moves 5 17 23]
"""
import argparse
from copy import deepcopy
import json
import re
from go_rules import Position, COLS, sequence_position


def side(c):
    return {'Black': 'B', 'White': 'W'}.get(c, c)


def opponent(c):
    return 'W' if side(c) == 'B' else 'B'


def coordinate(p, mv):
    xy = p.xy(mv)
    if xy is None: return {'move': mv, 'location': 'pass'}
    x, y = xy
    vertical = 'upper' if y < (p.sy-1)/2 else 'lower' if y > (p.sy-1)/2 else 'middle'
    horizontal = 'left' if x < (p.sx-1)/2 else 'right' if x > (p.sx-1)/2 else 'centre'
    return {'move': mv, 'location': f'{vertical} {horizontal}', 'row_from_top': y+1,
            'row_from_bottom': p.sy-y, 'column_from_left': x+1,
            'text': f'{mv} is {vertical} {horizontal}, row {y+1} from the top.'}


def gtp(p, xy):
    return COLS[xy[0]] + str(p.sy-xy[1])


def group_fact(p, anchor):
    xy = p.xy(anchor)
    if xy not in p.board: return {'anchor': anchor, 'present': False}
    stones, libs = p.group(xy)
    return {'anchor': anchor, 'present': True, 'color': p.board[xy],
            'stones': sorted(gtp(p, x) for x in stones),
            'liberties': sorted(gtp(p, x) for x in libs), 'liberty_count': len(libs)}


def sequence_fact(parsed, seq):
    sequence_position(parsed, seq)  # Includes artificial passes and original history.
    c = side(seq['first_to_move'])
    plies = [{'ply': i+1, 'color': c if i % 2 == 0 else opponent(c), 'move': m}
             for i, m in enumerate(seq['moves'])]
    return {'from_turn': seq['from_turn'], 'kind': 'variation, not actual game moves',
            'source': seq.get('source'), 'profile': seq.get('profile'),
            'score_black': seq.get('score_black'), 'plies': plies,
            'text': ' → '.join(f"{x['ply']} {x['color']} {x['move']}" for x in plies)}


def policy_fact(c):
    mv = c['played_move']
    out = {'applies_to_move': mv, 'kind': 'model probability, not observed human frequency'}
    for label, key in [('engine', 'policy_prob'), ('student', 'human_prob'), ('target', 'target_prob')]:
        if c.get(key) is not None:
            out[label] = {'move': mv, 'probability': c[key], 'percent': round(c[key]*100, 3)}
    # Older format-5 reports retain target policy only in a generated hint. Do not invent precision.
    if 'target' not in out:
        for hint in c.get('hints', []):
            match = re.search(r'stronger target profile play this move ([\d.]+)%', hint)
            if match:
                pct = float(match[1])
                out['target'] = {'move': mv, 'percent': pct, 'probability': pct/100,
                                 'precision': 'rounded report hint'}
                break
    if 'student' in out and 'target' in out:
        a, b = out['student']['percent'], out['target']['percent']
        # Compare at the coarser displayed precision when the target comes from an old hint.
        if out['target'].get('precision'): a = round(a, 1)
        direction = 'higher' if b > a else 'lower' if b < a else 'equal at reported precision'
        out['target_vs_student'] = direction
        out['comparison_text'] = (f"For played {mv}, target policy is {b:g}% versus student {a:g}% "
                                  f"({direction}). These are model probabilities.")
    return out


def history_facts(parsed):
    gi = parsed['game_info']; s = side(gi.get('student', 'B'))
    names = {c: gi.get(k) for c, k in [('B','black_player'), ('W','white_player')]}
    source = parsed.get('history') or []
    if not isinstance(source, list):
        return {'prior_games': [], 'possible_same_game_snapshots': [],
                'note': 'Legacy history table has no reliable identity; do not use it as current statistics.'}
    previous, snapshots, uncertain = [], [], []
    def identity(e):
        return (e.get('date'), e.get('student_name'), e.get('opponent_name'),
                e.get('board'), e.get('moves'), e.get('result'))
    current = {'date': gi.get('date'), 'student_name': names[s], 'opponent_name': names[opponent(s)],
               'board': f"{gi['board_size']}x{gi.get('board_size_y',gi['board_size'])}",
               'moves': len(parsed['moves']), 'result': gi.get('result')}
    seen = set()
    for entry in sorted(source, key=lambda e: e.get('analysed_at',''), reverse=True):
        key = identity(entry)
        # Without move hashes this is a possible match, not proven identity. Exclude conservatively.
        if key == identity(current): snapshots.append(deepcopy(entry)); continue
        if not entry.get('date') or not entry.get('student_name') or not entry.get('opponent_name'):
            uncertain.append(deepcopy(entry)); continue
        if key not in seen: previous.append(deepcopy(entry)); seen.add(key)
    return {'prior_games': list(reversed(previous)), 'possible_same_game_snapshots': snapshots,
            'unidentified_entries': uncertain,
            'note': 'Current stats come only from this report. Metadata matches may be earlier analyses of the same game; exclude from trends. Raw history remains in parsed.json.'}


def build_facts(parsed, selected=None):
    if parsed.get('reading_view_only'): raise ValueError('Use complete parsed.json, not brief.json')
    gi = parsed['game_info']; student = side(gi.get('student','B'))
    accuracy = deepcopy(parsed.get('summary',{}).get('accuracy',{}).get(student,{}))
    if accuracy.get('moves') and accuracy.get('top1_moves') is not None:
        accuracy['top1_percent'] = round(100*accuracy['top1_moves']/accuracy['moves'], 3)
    facts = {'student': student,
             'actual_moves': {str(m['number']): {'color': m['color'], 'move': m['move']} for m in parsed['moves']},
             'current': {'accuracy': accuracy, 'phases': deepcopy(parsed.get('arc',{}).get('phases',[]))},
             'history': history_facts(parsed), 'lessons': {}}
    recent = facts['history']['prior_games'][-10:]
    phase_values = {p['name']:p.get('student_mean_loss') for p in facts['current']['phases'] if 'name' in p}
    rows = []
    for label, key, value in [('Mean point loss', 'mean_loss', accuracy.get('mean_loss')),
            ('Opening loss', 'opening_loss', phase_values.get('opening')),
            ('Middlegame loss', 'middlegame_loss', phase_values.get('middlegame')),
            ('Endgame loss', 'endgame_loss', phase_values.get('endgame'))]:
        samples = [e[key] for e in recent if isinstance(e.get(key),(int,float))]
        if value is not None and samples:
            rows.append({'metric':label, 'this_game':f'{value:.3f}', 'recent_average':f'{sum(samples)/len(samples):.3f}'})
    facts['current']['progress_rows'] = rows
    facts['current']['prior_game_count'] = len(recent)
    for c in parsed.get('teaching',{}).get('candidates',[]):
        n = c['move_number']
        if selected is not None and n not in selected: continue
        if 'played_move' not in c: continue  # Legacy narrative fields remain usable without v5 probes.
        p = Position.from_parsed(parsed, n-1); e = c.get('evidence',{})
        seqs = {s['id']: sequence_fact(parsed,s) for s in e.get('sequences',[])}
        local = e.get('local_reading') or {}; trials = {}
        for t in local.get('trials',[]):
            seq = t.get('sequence'); key = seq['id'] if seq else str(len(trials))
            color = side(local['group_color'])
            trials[key] = {'defender': color, 'first_local_player': side(t['first']),
                          'ownership_for_defender': t.get('group_ownership_for_defender'),
                          'prediction_for_defender': t.get('prediction'),
                          'text': f"{side(t['first'])} plays first locally; {t.get('prediction','unavailable')} is a prediction for defender {color}, not a life/death proof."}
            if seq:
                seqs[key] = sequence_fact(parsed, seq)
                trials[key]['artificial_initial_pass'] = bool(seq['moves'] and seq['moves'][0]=='pass')
        rollouts = {}
        for r in e.get('rollout_comparisons',[]):
            d = r.get('best_minus_played_for_student')
            if not isinstance(d, (int,float)): continue
            fav = 'best' if d > 0 else 'played' if d < 0 else 'neither'
            rollouts[r['level']] = {'favours_start': fav, 'difference_for_student': d,
                'best_score_black': seqs.get('best_'+r['level'],{}).get('score_black'),
                'played_score_black': seqs.get('played_'+r['level'],{}).get('score_black'),
                'text': f"The {r['level']} sample comparison favours the {fav} start by {abs(d):g} points for {student}. This includes later choices; it is not the first move's causal value."}
        anchors = set((local.get('stones') or [])[:1]) | {s['anchor'] for s in c.get('status_changes',[])}
        groups = {}
        for anchor in sorted(anchors):
            entry = {'before': group_fact(p,anchor)}
            for label, mv in [('after_played', c['played_move']), ('after_best',c.get('preferred_move'))]:
                if mv:
                    q=p.clone(); q.play(mv); entry[label]=group_fact(q,anchor)
            groups[anchor] = entry
        facts['lessons'][str(n)] = {'actual': facts['actual_moves'][str(n)],
            'next_actual': facts['actual_moves'].get(str(n+1)),
            'coordinates': {m: coordinate(p,m) for m in (c['played_move'],c.get('preferred_move')) if m},
            'policy': policy_fact(c), 'local_trials': trials, 'groups': groups,
            'rollouts': rollouts, 'sequences': seqs,
            'ownership_limit': 'Ownership forecasts and nearby moves do not prove a local killing move, unconditional life/death, or sente.'}
    return facts


def pointer(data, path):
    if not isinstance(path,str) or not path.startswith('/'): raise ValueError('fact path must be a JSON pointer')
    for part in path[1:].split('/'):
        part = part.replace('~1','/').replace('~0','~')
        data = data[int(part)] if isinstance(data,list) else data[part]
    return data


def expand_facts(authored, facts):
    """Optional literal fact insertions preserve the numerical assertion through rendering."""
    if isinstance(authored,dict): return {k: expand_facts(v,facts) for k,v in authored.items()}
    if isinstance(authored,list): return [expand_facts(v,facts) for v in authored]
    if not isinstance(authored,str): return authored
    def substitute(m):
        value=pointer(facts,m[1])
        if isinstance(value,(dict,list)) or value is None: raise ValueError('fact token must select available text or a scalar')
        return str(value)
    return re.sub(r'\{\{fact:(/[^{}]+)\}\}',substitute,authored)


def main():
    ap=argparse.ArgumentParser(description=__doc__); ap.add_argument('parsed'); ap.add_argument('output'); ap.add_argument('--moves',type=int,nargs='+')
    args=ap.parse_args()
    with open(args.parsed) as f: parsed=json.load(f)
    with open(args.output,'w') as f: json.dump(build_facts(parsed,args.moves),f,indent=1,ensure_ascii=False)

if __name__=='__main__': main()
