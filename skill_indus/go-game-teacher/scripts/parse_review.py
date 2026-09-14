#!/usr/bin/env python3
"""Parse a go_teacher review markdown into structured JSON.

Default output is *brief*: everything from the summary and teaching-candidates sections, the
compact move list, and scalar per-move facts, but candidate tables and board diagrams only for
teaching candidates and praised moves. Pass --full to keep every table and diagram.

Usage: parse_review.py <input.md> <output.json> [--full]
"""

import json
import re
import sys

CATEGORIES = r'(best/excellent|good|inaccuracy|mistake|big mistake|blunder)'
MOVE = r'([A-T]\d+|pass)'


def section(text, heading):
    """Return the body of a '## heading' section (up to the next '## ')."""
    m = re.search(r'^## ' + re.escape(heading) + r'.*?\n(.*?)(?=^## |\Z)', text, re.DOTALL | re.MULTILINE)
    return m.group(1) if m else ''


def parse_game_info(text):
    info = {}
    body = section(text, 'Game information')
    for m in re.finditer(r'^\|\s*([^|]+?)\s*\|\s*(.+?)\s*\|\s*$', body, re.MULTILINE):
        key, val = m.group(1).strip().lower(), m.group(2).strip()
        if key == 'board size':
            info['board_size'] = int(re.search(r'(\d+)', val).group(1))
        elif key == 'komi used for analysis':
            try:
                info['komi'] = float(val)
            except ValueError:
                pass
        elif key == 'black':
            info['black_player'] = val
        elif key == 'white':
            info['white_player'] = val
        elif key == 'result recorded in sgf':
            info['result'] = val
        elif key == 'rules used for analysis':
            info['rules'] = val.split('(')[0].strip()
        elif key == 'number of moves':
            info['num_moves'] = int(val)
        elif key == 'handicap':
            info['handicap'] = int(val) if val.strip().isdigit() else 0
        elif key == 'date':
            info['date'] = val
        elif key == 'student':
            sm = re.match(r'(Black|White)\s*\((.*)\)\s*$', val)
            if sm:
                info['student'] = sm.group(1)[0]
                info['student_reason'] = sm.group(2)
            else:
                info['student'] = val[:1].upper() if val[:1].upper() in 'BW' else 'B'
        elif key == 'analysis strength':
            info['analysis_strength'] = val
    info.setdefault('student', 'B')
    return info


def parse_summary(text):
    summary = {'biggest_mistakes': [], 'turning_points': [], 'accuracy': {}}
    body = section(text, 'Summary')

    mistake_pattern = re.compile(
        r'\|\s*(\d+)\s*\|\s*' + MOVE + r'\s*\|\s*([\d.]+)\s*\|'
        r'\s*([\d.]+)%\s*→\s*([\d.]+)%\s*\|\s*' + CATEGORIES + r'\s*\|'
        r'\s*' + MOVE + r'\s*\|\s*(\w+)\s*\|')
    # The section has one table per player, introduced by "**Black (...)**" / "**White (...)**".
    current = None
    for line in body.split('\n'):
        hm = re.match(r'\*\*(Black|White)', line)
        if hm:
            current = hm.group(1)[0]
        mm = mistake_pattern.match(line.strip())
        if mm:
            summary['biggest_mistakes'].append({
                'player': current,
                'move_number': int(mm.group(1)),
                'played_move': mm.group(2),
                'point_loss': float(mm.group(3)),
                'winrate_before': mm.group(4) + '%',
                'winrate_after': mm.group(5) + '%',
                'category': mm.group(6),
                'preferred_move': mm.group(7),
                'phase': mm.group(8),
            })

    tp = re.search(r'### Turning points\s*\n(.*?)(?=\n###|\Z)', body, re.DOTALL)
    if tp:
        for line in tp.group(1).strip().split('\n'):
            line = line.strip().lstrip('- ').strip()
            if line and not line.startswith('No single move'):
                summary['turning_points'].append(line)

    acc = re.search(r'### Accuracy by player\s*\n(.*?)(?=\n###|\Z)', body, re.DOTALL)
    if acc:
        for m in re.finditer(r'^\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|\s*$', acc.group(1), re.MULTILINE):
            k = m.group(1).strip()
            if k.lower() in ('metric', '---') or k.startswith('-'):
                continue
            summary['accuracy'][k] = {'B': m.group(2).strip(), 'W': m.group(3).strip()}
    return summary


def parse_teaching(text):
    """The program's shortlist of teachable mistakes, with board facts and puzzle-ready stones."""
    out = {'student': None, 'student_reason': '', 'candidates': [], 'praise': []}
    body = section(text, 'Teaching candidates')
    if not body:
        return out
    sm = re.search(r'Student:\s*\*\*(Black|White)', body)
    if sm:
        out['student'] = sm.group(1)[0]
    rm = re.search(r'Student:\s*\*\*[^*]+\*\*\s*\((.*?)\)\.', body)
    if rm:
        out['student_reason'] = rm.group(1)

    row = re.compile(
        r'\|\s*(\d+)\s*\|\s*' + MOVE + r'\s*\|\s*(' + MOVE[1:-1] + r'|—)\s*\|\s*([\d.]+)\s*\|\s*([^|]*?)\s*\|'
        r'\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|')
    by_number = {}
    for m in row.finditer(body):
        decided = m.group(5)
        region = m.group(6)
        rm2 = re.match(r'(.+?)\s*\(~([\d.]+) pts\)', region)
        cap = re.match(r'yes, move (\d+)', m.group(8))
        c = {
            'move_number': int(m.group(1)),
            'played_move': m.group(2),
            'preferred_move': None if m.group(3) == '—' else m.group(3),
            'point_loss': float(m.group(4)),
            'decided_before': decided.startswith('yes'),
            'winrate_before': (re.search(r'([\d.]+%)', decided).group(1) if 'yes' in decided else None),
            'loss_region': rm2.group(1) if rm2 else None,
            'loss_region_points': float(rm2.group(2)) if rm2 else None,
            'better_move_is': m.group(7),
            'captured_at': int(cap.group(1)) if cap else None,
            'net_policy': m.group(9),
            'human_policy': m.group(10),
            'hints': [],
            'black_stones': [],
            'white_stones': [],
            'opponent_last_move': None,
            'player_color': None,
        }
        by_number[c['move_number']] = c
        out['candidates'].append(c)

    for blk in re.finditer(r'### Candidate: move (\d+) \((Black|White) ' + MOVE + r'\)\s*\n(.*?)(?=\n### |\Z)', body, re.DOTALL):
        n = int(blk.group(1))
        c = by_number.get(n)
        if not c:
            continue
        c['player_color'] = blk.group(2)[0]
        for line in blk.group(4).split('\n'):
            line = line.strip()
            if not line.startswith('- '):
                continue
            line = line[2:]
            pm = re.match(r'Position before the move \((Black|White) to play; opponent\'s last move (\S+)\): Black stones (.*?); White stones (.*?)\.$', line)
            if pm:
                c['opponent_last_move'] = None if pm.group(2) == 'none' else pm.group(2)
                c['black_stones'] = [] if pm.group(3) == 'none' else pm.group(3).split()
                c['white_stones'] = [] if pm.group(4) == 'none' else pm.group(4).split()
            elif line.startswith('Full entry'):
                continue
            else:
                c['hints'].append(line)

    pr = re.search(r'### Good moves worth praising\s*\n(.*)', body, re.DOTALL)
    if pr:
        for m in re.finditer(r'\|\s*(\d+)\s*\|\s*(Black|White)\s+' + MOVE + r'\s*\|\s*' + MOVE + r'\s*\|\s*([\d.]+)\s*\|\s*([\d.]+%)\s*\|', pr.group(1)):
            out['praise'].append({
                'move_number': int(m.group(1)),
                'player_color': m.group(2)[0],
                'move': m.group(3),
                'second_best': m.group(4),
                'gap_points': float(m.group(5)),
                'winrate_before': m.group(6),
            })
    return out


def parse_compact_moves(text):
    moves = []
    m = re.search(r'## Appendix: compact move list.*?```\s*\n(.*?)```', text, re.DOTALL)
    if not m:
        return moves
    pat = re.compile(r'\s*(\d+)\s+([BW])\s+(\S+)\s+loss\s+([\d.+-]+)\s+rank\s+(\S+)\s+wr\s+(\S+)\s+(\S+)')
    for line in m.group(1).strip().split('\n'):
        mm = pat.match(line)
        if mm:
            moves.append({
                'number': int(mm.group(1)),
                'color': mm.group(2),
                'move': mm.group(3),
                'point_loss': float(mm.group(4)),
                'rank': mm.group(5),
                'winrate_after': mm.group(6),
                'score_after': mm.group(7),
            })
    return moves


def parse_move_analysis(text):
    """Full entries ('### Move N') and one-line brief entries ('- **Move N** ...')."""
    details = {}
    body = section(text, 'Move-by-move analysis')

    brief = re.compile(r'^- \*\*Move (\d+)\*\* (Black|White) (\S+): loss ([\d.+-]+) \(([^)]+)\), then Black ([\d.]+%) / (\S+)\.', re.MULTILINE)
    for m in brief.finditer(body):
        n = int(m.group(1))
        details[n] = {
            'number': n, 'color': m.group(2)[0], 'move': m.group(3), 'brief': True,
            'point_loss': float(m.group(4)), 'rank_info': m.group(5),
            'winrate_after': m.group(6), 'score_after': m.group(7), 'candidates': [],
        }

    for sec in re.split(r'(?=^### Move \d+:)', body, flags=re.MULTILINE):
        h = re.match(r'### Move (\d+): (Black|White) (\S+)', sec)
        if not h:
            continue
        n = int(h.group(1))
        coord = h.group(3)
        d = {'number': n, 'color': h.group(2)[0], 'move': coord, 'brief': False, 'candidates': []}

        m = re.search(r'Point loss for \w+:\s*([\d.+-]+)\s*pts\s*\(([^)]+)\)', sec)
        if m:
            d['point_loss'] = float(m.group(1))
            d['rank_info'] = m.group(2)
        m = re.search(r'Verdict:\s*\*\*([^*]+)\*\*', sec)
        if m:
            d['verdict'] = m.group(1).strip()
        m = re.search(r'Phase:\s*(\w+)\.', sec)
        if m:
            d['phase'] = m.group(1)
        m = re.search(r'winrate\s*([\d.]+)%\s*→\s*([\d.]+)%.*?score\s*(\S+)\s*→\s*(\S+)', sec)
        if m:
            d['winrate_before'] = m.group(1) + '%'
            d['winrate_after'] = m.group(2) + '%'
            d['score_before'] = m.group(3)
            d['score_after'] = m.group(4).rstrip('.')
        m = re.search(r'KataGo preferred\s+\*\*' + MOVE + r'\*\*\s*\(([\d.]+%),\s*(\S+)\)', sec)
        if m:
            d['preferred_move'] = m.group(1)
            d['preferred_winrate'] = m.group(2)
            d['preferred_score'] = m.group(3).rstrip(')').rstrip(',')
        m = re.search(r'KataGo preferred\s+\*\*(?:' + MOVE[1:-1] + r')\*\*.*?expecting:\s*((?:[A-T]\d+|pass)(?:\s+(?:[A-T]\d+|pass))*)', sec)
        if m:
            d['preferred_pv'] = m.group(1).split()
        m = re.search(r'expected continuation after\s+' + re.escape(coord) + r':\s*((?:[A-T]\d+|pass)(?:\s+(?:[A-T]\d+|pass))*)', sec)
        if m:
            d['played_pv'] = m.group(1).split()
        m = re.search(r'Network policy for \S+: (<?[\d.]+%) \(#(\d+) over the whole board\)\.(?: Human policy: (<?[\d.]+%) \(#(\d+)\)(?:, most common human move (\S+) \(([\d.]+)%\))?\.)?', sec)
        if m:
            d['policy_prob'] = m.group(1)
            d['policy_rank'] = int(m.group(2))
            if m.group(3):
                d['human_policy_prob'] = m.group(3)
                d['human_policy_rank'] = int(m.group(4))
                if m.group(5):
                    d['human_top_move'] = m.group(5)
                    d['human_top_prob'] = m.group(6) + '%'
        m = re.search(r'Comment in the game record: (.*)', sec)
        if m:
            d['comment'] = m.group(1).strip()

        cm = re.search(r'Candidates in the position before this move:\s*\n(.*?)(?=\nPosition after|\n###|\n- \*\*Move|\Z)', sec, re.DOTALL)
        if cm:
            cand = re.compile(
                r'\|\s*(\d+)\s*\|\s*' + MOVE + r'(\s*←\s*played)?\s*\|\s*([\d.]+)%\s*\|\s*(\S+)\s*\|\s*(best|[\d.+-]+)\s*\|\s*(\d+)\s*\|\s*([\d.]+)%\s*\|\s*(.*?)\s*\|')
            for c in cand.finditer(cm.group(1)):
                pv = c.group(9).strip()
                d['candidates'].append({
                    'rank': int(c.group(1)),
                    'move': c.group(2),
                    'played': c.group(3) is not None,
                    'winrate': c.group(4) + '%',
                    'score': c.group(5),
                    'loss_vs_best': 0.0 if c.group(6) == 'best' else float(c.group(6)),
                    'visits': int(c.group(7)),
                    'policy': c.group(8) + '%',
                    'pv': [] if pv in ('—', '') else pv.split(),
                })
        dm = re.search(r'Position after move \d+:\s*\n```\s*\n(.*?)```', sec, re.DOTALL)
        if dm:
            d['board_diagram'] = dm.group(1).strip()
        details[n] = d
    return details


def parse_review(text, full=False):
    result = {
        'game_info': parse_game_info(text),
        'summary': parse_summary(text),
        'teaching': parse_teaching(text),
        'moves': parse_compact_moves(text),
        'move_details': parse_move_analysis(text),
    }
    if result['teaching']['student']:
        result['game_info']['student'] = result['teaching']['student']
    if not full:
        keep = {c['move_number'] for c in result['teaching']['candidates']} | {p['move_number'] for p in result['teaching']['praise']}
        for n, d in result['move_details'].items():
            if n not in keep:
                d.pop('board_diagram', None)
                d['candidates'] = d['candidates'][:3]
                for c in d['candidates']:
                    c['pv'] = c['pv'][:4]
    return result


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    full = '--full' in sys.argv
    if len(args) != 2:
        print(f"Usage: {sys.argv[0]} <input.md> <output.json> [--full]")
        sys.exit(1)
    with open(args[0], 'r', encoding='utf-8') as f:
        text = f.read()
    result = parse_review(text, full=full)
    with open(args[1], 'w', encoding='utf-8') as f:
        json.dump(result, f, indent=1, ensure_ascii=False)
    t = result['teaching']
    print(f"Student: {result['game_info'].get('student')}  moves: {len(result['moves'])}  "
          f"teaching candidates: {len(t['candidates'])}  praised: {len(t['praise'])}  "
          f"move details: {len(result['move_details'])} ({'full' if full else 'brief'})", file=sys.stderr)


if __name__ == '__main__':
    main()
