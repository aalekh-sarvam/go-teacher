#!/usr/bin/env python3
"""Parse a go_teacher review markdown into structured JSON.

Default output is *brief*: everything from the summary and teaching-candidates sections, the
compact move list, and scalar per-move facts, but candidate tables and board diagrams only for
teaching candidates and praised moves. Pass --full to keep every table and diagram.

Usage: parse_review.py <input.md> <output.json> [--full] [--brief-output brief.json]
       [--facts-output facts.json [--moves N ...]]
"""

import json
import re
import sys

CATEGORIES = r'(best/excellent|good|inaccuracy|mistake|big mistake|blunder)'
MOVE = r'([A-T]\d+|pass)'
SUPPORTED_FORMATS = {2, 3, 4, 5}   # "Report format: N" line written by go_teacher
SUPPORTED_EVIDENCE_VERSIONS = {1, 2}   # evidence_version inside the format-5 JSON block; 2 adds praise
                                       # kinds/ratings, student_profile, *_with_colours and stones_captured,
                                       # and (additively) search_coverage / per-candidate coverage fields
SUPPORTED_COVERAGE_VERSIONS = {1}      # search_coverage.version; other versions are passed through as unknown

# Deep-coverage vocabularies (evidence_version 2 reports written after the two-pass verification work).
EVALUATION_STATUSES = ('complete', 'incomplete', 'disabled', 'unknown')        # evaluation_coverage.status
POSITION_STATUSES = ('complete', 'pending', 'disabled', 'unknown')             # evaluation_coverage.before/after.status
INVESTIGATION_STATUSES = ('available', 'partial', 'not_selected', 'disabled', 'unavailable')   # investigation_coverage.status
# evidence.unavailable keys that mean "no human/local/what-if investigations were run for this candidate".
# 'deeper_search' is the legacy spelling: it says nothing about the depth of the main evaluation.
NO_INVESTIGATIONS_KEYS = ('teaching_investigations', 'deeper_search')


def report_format(text):
    m = re.search(r'^Report format:\s*(\d+)', text, re.MULTILINE)
    return int(m.group(1)) if m else 2   # reports before the line existed


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
            info['two_pass'] = val.startswith('two-pass')
        elif key == 'deeply re-analysed positions':
            m2 = re.match(r'(\d+)', val)
            info['deepened_positions'] = int(m2.group(1)) if m2 else 0
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

    header = re.search(r'^\|\s*Move\s*\|.*$', body, re.MULTILINE)
    cols = [c.strip() for c in header.group(0).strip('|').split('|')] if header else []
    by_number = {}
    for line in body.split('\n'):
        line = line.strip()
        if not line.startswith('|') or line.startswith('|--') or not cols:
            continue
        cells = [c.strip() for c in line.strip('|').split('|')]
        if len(cells) != len(cols) or not cells[0].isdigit():
            continue
        d = dict(zip(cols, cells))
        decided = d.get('Decided?', '')
        region = d.get('Where the points went', '')
        rm2 = re.match(r'(.+?)\s*\(~([\d.]+) pts\)', region)
        cap = re.match(r'yes, move (\d+)', d.get('Stone captured?', ''))
        better = d.get('Better', '—')
        c = {
            'move_number': int(d['Move']),
            'played_move': d.get('Played'),
            'preferred_move': None if better == '—' else better,
            'point_loss': float(d.get('Loss', '0') or 0),
            'theme': d.get('Theme'),
            'decided_before': decided.startswith('yes'),
            'winrate_before': (re.search(r'([\d.]+%)', decided).group(1) if 'yes' in decided else None),
            'loss_region': rm2.group(1) if rm2 else None,
            'loss_region_points': float(rm2.group(2)) if rm2 else None,
            'better_move_is': d.get('Better move is'),
            'captured_at': int(cap.group(1)) if cap else None,
            'net_policy': d.get('Net policy'),
            'human_policy': d.get('Human policy'),
            'refutation': [],
            'refutation_score': None,
            'better_line': [],
            'chain_before': [],
            'chain_after': [],
            'status_changes': [],
            'phase': None,
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
            bl = re.match(r'Better line \((Black|White) first\): (.*)$', line)
            rf = re.match(r'What the move allows \((Black|White) first\): (.*?) → ([BW]\+[\d.]+)$', line)
            ch = re.match(r'Chain in this area — before: (.*); after: (.*)\.$', line)
            th = re.match(r'Theme: (.*?)\. Phase: (\w+)\.$', line)
            sc = re.match(r'This move changed the status of the (Black|White) group at (\S+) \((\d+) stones?\): (\w+) → (\w+)\.$', line)
            tg = re.match(r'Players at the stronger target profile play this move (.*?) of the time \(#(\d+)\)(?:; their most common move here is (\S+) \((\d+)%\))?\.$', line)
            tm = re.match(r'Played (?:in|after) ([\d.]+) s', line)
            if pm:
                c['opponent_last_move'] = None if pm.group(2) == 'none' else pm.group(2)
                c['black_stones'] = [] if pm.group(3) == 'none' else pm.group(3).split()
                c['white_stones'] = [] if pm.group(4) == 'none' else pm.group(4).split()
            elif bl:
                c['better_line'] = bl.group(2).split()
            elif rf:
                c['refutation'] = rf.group(2).split()
                c['refutation_score'] = rf.group(3)
            elif ch:
                c['chain_before'] = parse_chain(ch.group(1))
                c['chain_after'] = parse_chain(ch.group(2))
            elif th:
                c['theme'] = th.group(1)
                c['phase'] = th.group(2)
            elif sc:
                c['status_changes'].append({'group_color': sc.group(1)[0], 'anchor': sc.group(2), 'stones': int(sc.group(3)), 'from': sc.group(4), 'to': sc.group(5)})
                c['hints'].append(line)
            elif tg:
                c['target_policy'] = {'prob': tg.group(1), 'rank': int(tg.group(2)), 'top_move': tg.group(3), 'top_prob': (tg.group(4) + '%') if tg.group(4) else None}
                c['hints'].append(line)
            elif tm:
                c['time_spent_seconds'] = float(tm.group(1))
                c['fast'] = 'fastest' in line
                c['hints'].append(line)
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


def parse_chain(text):
    """'7 Black F5 (loss 3.3), 9 Black D3 (loss 9.3; captured at move 16)' -> list of dicts."""
    out = []
    if text.strip() == 'none':
        return out
    for m in re.finditer(r'(\d+) (Black|White) (\S+) \(loss ([\d.+-]+)(?:; ([^)]*))?\)', text):
        out.append({'move_number': int(m.group(1)), 'color': m.group(2)[0], 'move': m.group(3),
                    'point_loss': float(m.group(4)), 'note': m.group(5) or ''})
    return out


def parse_arc(text):
    """The 'Game arc facts' section: per-phase numbers and life-and-death changes."""
    out = {'phases': [], 'status_changes': []}
    body = section(text, 'Game arc facts')
    if not body:
        return out
    for m in re.finditer(
            r'^\|\s*(opening|middlegame|endgame)\s*\|\s*(\d+)–(\d+)\s*\|\s*([\d.]+)% → ([\d.]+)%\s*\|\s*(\S+) → (\S+)\s*\|'
            r'\s*([\d.]+)\s*\|\s*([\d.]+)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*(\d+)%\s*\|\s*([^|]*?)\s*\|', body, re.MULTILINE):
        def worst(s):
            w = re.match(r'(\d+) (\S+) \(([\d.]+)\)', s)
            return {'move_number': int(w.group(1)), 'move': w.group(2), 'point_loss': float(w.group(3))} if w else None
        hot = [{'region': h.group(1).strip(), 'points': float(h.group(2))} for h in re.finditer(r'([a-z ]+?) \((\d+)\)', m.group(13))]
        out['phases'].append({
            'phase': m.group(1), 'first_move': int(m.group(2)), 'last_move': int(m.group(3)),
            'winrate_start': m.group(4) + '%', 'winrate_end': m.group(5) + '%',
            'score_start': m.group(6), 'score_end': m.group(7),
            'student_mean_loss': float(m.group(8)), 'opponent_mean_loss': float(m.group(9)),
            'student_worst': worst(m.group(10)), 'opponent_worst': worst(m.group(11)),
            'student_top1_rate': m.group(12) + '%', 'hot_regions': hot,
        })
    for m in re.finditer(r'^\|\s*(\d+)\s*\|\s*(Black|White)\s*\|\s*(Black|White) (\S+)\s*\|\s*(\d+)\s*\|\s*(\w+)\s*\|\s*(\w+)\s*\|', body, re.MULTILINE):
        out['status_changes'].append({'move_number': int(m.group(1)), 'mover': m.group(2)[0], 'group_color': m.group(3)[0],
                                      'anchor': m.group(4), 'stones': int(m.group(5)), 'from': m.group(6), 'to': m.group(7)})
    return out


def parse_openings(text):
    """'## Opening patterns': per corner, the described moves and the first deviation."""
    out = []
    body = section(text, 'Opening patterns')
    for blk in re.finditer(r'^### (.+?) corner: (.*?)\n(.*?)(?=^### |\Z)', body, re.DOTALL | re.MULTILINE):
        corner = {'corner': blk.group(1), 'summary': blk.group(2).strip(), 'moves': [], 'first_deviation': None}
        for m in re.finditer(r'^- Move (\d+) (Black|White) (\S+): (.*?) \(loss ([\d.+-]+), (.*?)\)$', blk.group(3), re.MULTILINE):
            corner['moves'].append({'move_number': int(m.group(1)), 'color': m.group(2)[0], 'move': m.group(3),
                                    'description': m.group(4), 'point_loss': float(m.group(5)), 'verdict': m.group(6)})
        d = re.search(r'First deviation: move (\d+) (\S+) \(KataGo preferred (\S+), ([\d.]+) points\)', blk.group(3))
        if d:
            corner['first_deviation'] = {'move_number': int(d.group(1)), 'move': d.group(2), 'preferred': d.group(3), 'point_loss': float(d.group(4))}
        out.append(corner)
    return out


def parse_history(text):
    """'## Compared with the student's earlier games': this game vs recent averages, recurring themes, past games."""
    body = section(text, "Compared with the student's earlier games")
    if not body:
        return None
    out = {'earlier_games': 0, 'metrics': {}, 'recurring_themes': [], 'games': []}
    m = re.search(r'The student has (\d+) earlier analysed games', body)
    if m:
        out['earlier_games'] = int(m.group(1))
    for m in re.finditer(r'^\|\s*([^|]+?)\s*\|\s*([\d.%]+)\s*\|\s*([\d.%]+)\s*\|\s*$', body, re.MULTILINE):
        if m.group(1) not in ('Metric', 'Game'):
            out['metrics'][m.group(1)] = {'this_game': m.group(2), 'recent_average': m.group(3)}
    m = re.search(r'Recurring themes in recent games \(teaching candidates per theme\): (.*?)\.$', body, re.MULTILINE)
    if m:
        out['recurring_themes'] = [{'theme': t.group(1).strip(), 'count': int(t.group(2))} for t in re.finditer(r'([^,()]+?) \((\d+)\)', m.group(1))]
    for m in re.finditer(r'^\|\s*([^|]+?)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*([\d.]+)\s*\|\s*(\d+)%\s*\|\s*$', body, re.MULTILINE):
        if m.group(1) != 'Game':
            out['games'].append({'game': m.group(1), 'date': m.group(2), 'opponent': m.group(3), 'result': m.group(4),
                                 'mean_loss': float(m.group(5)), 'top1_rate': m.group(6) + '%'})
    return out


def parse_time_and_loss(text):
    """'### Time and loss' in the Summary: fast vs slow moves per player."""
    body = section(text, 'Summary')
    m = re.search(r'### Time and loss\s*\n(.*?)(?=\n###|\Z)', body, re.DOTALL)
    if not m:
        return None
    out = {'median_seconds': None, 'players': {}}
    med = re.search(r'median thinking time \(([\d.]+) s\)', m.group(1))
    if med:
        out['median_seconds'] = float(med.group(1))
    for r in re.finditer(r'^\|\s*(Black|White)\s*\|\s*(\d+)\s*\|\s*([\d.]+)\s*\|\s*(\d+)\s*\|\s*([\d.]+)\s*\|', m.group(1), re.MULTILINE):
        out['players'][r.group(1)[0]] = {'fast_moves': int(r.group(2)), 'fast_mean_loss': float(r.group(3)), 'slow_moves': int(r.group(4)), 'slow_mean_loss': float(r.group(5))}
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
        h = re.match(r'### Move (\d+): (Black|White) (\S+?)( ◆)?\s*$', sec, re.MULTILINE)
        if not h:
            continue
        n = int(h.group(1))
        coord = h.group(3)
        d = {'number': n, 'color': h.group(2)[0], 'move': coord, 'brief': False, 'deep': h.group(4) is not None, 'candidates': []}

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
        m = re.search(r'Network policy for \S+: (<?[\d.]+%) \(#(\d+) over the whole board\)\.(?: Human policy: (<?[\d.]+%) \(#(\d+)\)(?:, most common human move (\S+) \(([\d.]+)%\))?\.)?(?: Target profile: (<?[\d.]+%) \(#(\d+)\)(?:, most common move (\S+) \(([\d.]+)%\))?\.)?', sec)
        if m:
            d['policy_prob'] = m.group(1)
            d['policy_rank'] = int(m.group(2))
            if m.group(7):
                d['target_policy_prob'] = m.group(7)
                d['target_policy_rank'] = int(m.group(8))
                if m.group(9):
                    d['target_top_move'] = m.group(9)
                    d['target_top_prob'] = m.group(10) + '%'
            if m.group(3):
                d['human_policy_prob'] = m.group(3)
                d['human_policy_rank'] = int(m.group(4))
                if m.group(5):
                    d['human_top_move'] = m.group(5)
                    d['human_top_prob'] = m.group(6) + '%'
        m = re.search(r'Comment in the game record: (.*)', sec)
        if m:
            d['comment'] = m.group(1).strip()
        m = re.search(r'Time spent on this move: ([\d.]+) s\.', sec)
        if m:
            d['time_spent_seconds'] = float(m.group(1))

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
    fmt = report_format(text)
    if fmt not in SUPPORTED_FORMATS:
        raise SystemExit(f"This report uses format {fmt}; this parser understands {sorted(SUPPORTED_FORMATS)}. "
                         "Update the go-game-teacher skill (or go_teacher) so the versions match.")
    if fmt == 5:
        blocks = re.findall(r'^```go-teacher-evidence\s*\n(.*?)\n```[ \t]*$', text, re.MULTILINE | re.DOTALL)
        if len(blocks) != 1:
            raise ValueError('Format 5 requires exactly one complete evidence block')
        result = json.loads(blocks[0])
        if result.get('report_format') != 5:
            raise ValueError('Evidence block report_format does not match the report header')
        ev = result.get('evidence_version')
        if ev not in SUPPORTED_EVIDENCE_VERSIONS:
            raise SystemExit(f"This report's evidence block is evidence_version {ev!r}; this parser understands "
                             f"{sorted(SUPPORTED_EVIDENCE_VERSIONS)}. Update the go-game-teacher skill (or go_teacher) so the versions match.")
        # Version 2 fields (teaching.praise[].kind/rating/note, student_profile, *_with_colours,
        # moves[].stones_captured) are additive and passed through untouched. So are the deep-coverage
        # fields (top-level search_coverage, candidates[].evaluation_coverage / investigation_coverage,
        # evidence.unavailable.teaching_investigations, interpretation.coverage); read them through
        # coverage_summary(). Reports without them have unknown coverage, never incomplete or verified.
        if result['game_info']['num_moves'] != len(result['moves']):
            raise ValueError('Evidence move count mismatch')
        for i,m in enumerate(result['moves'],1):
            if m['number'] != i or m['color'] not in ('B','W'):
                raise ValueError('Invalid move order or colour')
        return result
    result = {
        'report_format': fmt,
        'game_info': parse_game_info(text),
        'summary': parse_summary(text),
        'teaching': parse_teaching(text),
        'arc': parse_arc(text),
        'openings': parse_openings(text),
        'history': parse_history(text),
        'time_and_loss': parse_time_and_loss(text),
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


def candidate_coverage(c, coverage_version=1):
    """Derived coverage for one teaching candidate. Missing fields are 'unknown' (legacy=True), never
    'incomplete' or 'complete'. evidence.unavailable.teaching_investigations or the legacy
    evidence.unavailable.deeper_search only mean 'investigations not_selected'."""
    ev = c.get('evaluation_coverage') if isinstance(c.get('evaluation_coverage'), dict) else {}
    inv = c.get('investigation_coverage') if isinstance(c.get('investigation_coverage'), dict) else {}
    unavailable = (c.get('evidence') or {}).get('unavailable') or {}
    legacy = not ev and not inv
    known = coverage_version in SUPPORTED_COVERAGE_VERSIONS
    e_status = ev.get('status') if known and ev.get('status') in EVALUATION_STATUSES else 'unknown'
    def visits(side):
        pos = ev.get(side) if isinstance(ev.get(side), dict) else {}
        v = pos.get('visits')
        return v if known and isinstance(v, int) and not isinstance(v, bool) else None
    if known and inv.get('status') in INVESTIGATION_STATUSES:
        i_status = inv['status']
    elif any(k in unavailable for k in NO_INVESTIGATIONS_KEYS):
        i_status = 'not_selected'
    else:
        i_status = 'unknown'
    return {'move_number': c.get('move_number'), 'evaluation_status': e_status,
            'before_visits': visits('before'), 'after_visits': visits('after'),
            'investigation_status': i_status, 'legacy': legacy}


def coverage_summary(parsed):
    """[{move_number, evaluation_status, before_visits, after_visits, investigation_status, legacy}]
    for every teaching candidate, in report order. Works for every supported format: formats 2-4 and
    evidence_version 1 give evaluation_status 'unknown' and legacy True throughout."""
    sc = parsed.get('search_coverage') if isinstance(parsed.get('search_coverage'), dict) else {}
    version = sc.get('version', 1)
    return [candidate_coverage(c, version) for c in parsed.get('teaching', {}).get('candidates', [])]


def main():
    import argparse
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('input'); ap.add_argument('output'); ap.add_argument('--full', action='store_true')
    ap.add_argument('--brief-output', help='Optional prose-model reading view; do not use it to generate HTML')
    ap.add_argument('--facts-output', help='Derived facts for the teaching agent; no additional user upload')
    ap.add_argument('--moves', type=int, nargs='+', metavar='N',
                    help='Build lesson facts only for these teaching moments (default: all); praise_facts are always built')
    args = ap.parse_args()
    with open(args.input, encoding='utf-8') as f: text = f.read()
    result = parse_review(text, full=args.full)
    with open(args.output, 'w', encoding='utf-8') as f:
        json.dump(result, f, indent=1, ensure_ascii=False)
    if args.brief_output:
        from lesson_contract import brief
        with open(args.brief_output,'w',encoding='utf-8') as f: json.dump(brief(result),f,indent=1,ensure_ascii=False)
    if args.facts_output:
        from teaching_facts import build_facts
        with open(args.facts_output,'w',encoding='utf-8') as f: json.dump(build_facts(result,args.moves),f,indent=1,ensure_ascii=False)
    elif args.moves:
        print('--moves only affects --facts-output; nothing to select', file=sys.stderr)
    full = args.full
    t = result['teaching']
    cov = coverage_summary(result)
    complete = sum(1 for c in cov if c['evaluation_status'] == 'complete')
    coverage_note = f"deep evaluation complete: {complete}/{len(cov)}" if any(not c['legacy'] for c in cov) else 'coverage not recorded'
    print(f"Report format {result['report_format']}  evidence v{result.get('evidence_version','-')}  Student: {result['game_info'].get('student')}  moves: {len(result['moves'])}  "
          f"teaching candidates: {len(t['candidates'])}  praised: {len(t['praise'])}  "
          f"move details: {len(result['move_details'])} ({'full' if full else 'brief'})  {coverage_note}", file=sys.stderr)


if __name__ == '__main__':
    main()
