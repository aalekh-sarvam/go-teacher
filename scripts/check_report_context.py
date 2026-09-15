#!/usr/bin/env python3
"""Check additive report changes against saved analysis; no engine or agent calls.

Usage: python3 scripts/check_report_context.py before.md after.md analysis.json
"""
import argparse
import json
import math
import statistics
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'skill_indus/go-game-teacher/scripts'))
from parse_review import parse_review
from lesson_contract import brief


def preserved(old, new, path='evidence'):
    """Every old field/value must remain; only new object keys may be added."""
    if isinstance(old, dict):
        assert isinstance(new, dict), path
        for key, value in old.items():
            assert key in new, f'{path}.{key} removed'
            preserved(value, new[key], f'{path}.{key}')
    elif isinstance(old, list):
        assert isinstance(new, list) and len(old) == len(new), path
        for i, (left, right) in enumerate(zip(old, new)):
            preserved(left, right, f'{path}[{i}]')
    else:
        assert old == new, f'{path} changed: {old!r} -> {new!r}'


def near(actual, expected):
    assert actual is not None and abs(actual - expected) <= 0.000501, (actual, expected)


def valid_time(review):
    value = review.get('time_spent')
    return value if value is not None and math.isfinite(value) and value >= 0 else None


def check(before, after, source):
    old, new = parse_review(before), parse_review(after)
    preserved(old, new)
    # Stronger than subset checks for the material the teacher already relied on.
    for key in ('teaching', 'arc', 'openings', 'history', 'profiles', 'rank_fit',
                'missed_opportunities', 'endgame_values', 'provenance'):
        assert old[key] == new[key], f'{key} changed'
    assert len(new['moves']) == len(source['reviews']) == len(source['game']['moves'])
    for move, review, sgf_move in zip(new['moves'], source['reviews'], source['game']['moves']):
        assert move['number'] == review['number']
        assert move['color'] == review['color'][0] == sgf_move['color'][0]
        assert move['move'] == review['mv']
        near(move['point_loss'], review['point_loss'])
        near(move['score_black'], review['score_after'])
        seconds = valid_time(review)
        if seconds is None:
            assert 'time_spent_seconds' not in move
        else:
            near(move['time_spent_seconds'], seconds)
    initial = next((t for t in source['turns'] if t['turn'] == 0), None)
    if initial:
        near(new['initial_position']['score_black'], initial['score_lead'])
        near(new['initial_position']['winrate_black'], initial['winrate'])
        assert new['initial_position']['visits'] == initial['visits']
    else:
        assert new['initial_position'] is None
    categories = {'Best':'best/excellent', 'Good':'good', 'Inaccuracy':'inaccuracy',
                  'Mistake':'mistake', 'BigMistake':'big mistake', 'Blunder':'blunder'}
    for color in ('B', 'W'):
        reviews = [r for r in source['reviews'] if r['color'][0] == color]
        losses = [max(0, r['point_loss']) for r in reviews]
        stats = new['summary']['accuracy'][color]
        assert stats['moves'] == len(reviews)
        near(stats['total_loss'], sum(losses))
        near(stats['mean_loss'], statistics.mean(losses) if losses else 0)
        if losses:
            near(stats['median_loss'], statistics.median(losses))
        else:
            assert stats['median_loss'] is None
        assert stats['ranked_moves'] == sum(r['rank'] is not None for r in reviews)
        assert stats['top1_moves'] == sum(r['rank'] == 0 for r in reviews)
        assert stats['top3_moves'] == sum(r['rank'] is not None and r['rank'] < 3 for r in reviews)
        assert stats['category_counts'] == {label:sum(r['category']==key for r in reviews)
                                            for key,label in categories.items()}
        timed = [r for r in reviews if valid_time(r) is not None]
        timing = new['summary']['timing']['players'][color]
        assert timing['timed_moves'] == len(timed)
        assert timing['untimed_moves'] == len(reviews)-len(timed)
        if timed:
            split = statistics.median(valid_time(r) for r in timed)
            near(timing['median_seconds'], split)
            near(timing['mean_seconds'], statistics.mean(valid_time(r) for r in timed))
            for below, name in ((True,'at_or_below_median'), (False,'above_median')):
                group = [max(0,r['point_loss']) for r in timed if (valid_time(r)<=split)==below]
                assert timing[name]['moves'] == len(group)
                if group:
                    near(timing[name]['mean_loss'], statistics.mean(group))
                else:
                    assert timing[name]['mean_loss'] is None
        else:
            assert 'median_seconds' not in timing
    expected_timing = 'available' if any(valid_time(r) is not None for r in source['reviews']) else 'unavailable'
    assert new['summary']['timing']['status'] == expected_timing
    student = new['game_info']['student']
    opponent = sorted([r for r in source['reviews'] if r['color'][0] != student and r['point_loss'] >= 2],
                      key=lambda r:-r['point_loss'])[:6]
    n = len(new['moves'])
    context = new['context_moves']
    assert context['opponent_mistakes'] == [r['number'] for r in opponent]
    assert context['checkpoints'] == list(range(50 if n > 200 else 25, n+1, 50 if n > 200 else 25))
    assert context['last_move'] == (n or None)
    expected = {c['move_number'] for c in new['teaching']['candidates'] + new['teaching']['praise']}
    expected.update(context['opponent_mistakes'] + context['checkpoints'])
    if n:
        expected.add(n)
    assert set(map(int, new['move_details'])) == expected
    for r in source['reviews']:
        if r['number'] not in expected:
            continue
        detail = new['move_details'][str(r['number'])]
        assert detail['player'] == r['color'][0] and detail['move'] == r['mv']
        assert len(detail['candidates']) == len(r['alternatives'])
        for candidate, original in zip(detail['candidates'], r['alternatives']):
            assert candidate['move'] == original['move'] and candidate['pv'] == original['pv']
            assert candidate['visits'] == original['visits']
            near(candidate['score_black'], original['score_lead'])
    view = brief(new)
    for key in ('moves', 'initial_position', 'context_moves', 'summary'):
        assert view[key] == new[key], f'brief lost {key}'
    before_bytes, after_bytes = len(before.encode()), len(after.encode())
    compact_size = lambda data: len(json.dumps(data, separators=(',',':'), ensure_ascii=False).encode())
    return dict(moves=n, teaching=len(new['teaching']['candidates']),
                detailed_moves_before=len(old['move_details']), detailed_moves_after=len(new['move_details']),
                before_bytes=before_bytes, after_bytes=after_bytes,
                added_bytes=after_bytes-before_bytes,
                added_percent=round((after_bytes/before_bytes-1)*100,1),
                timeline_added_bytes=compact_size(new['moves'])-compact_size(old['moves']),
                supporting_entries_added_bytes=compact_size(new['move_details'])-compact_size(old['move_details']),
                existing_evidence='all preserved', timing=expected_timing)


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('before', type=Path)
    ap.add_argument('after', type=Path)
    ap.add_argument('analysis', type=Path)
    args = ap.parse_args()
    print(json.dumps(check(args.before.read_text(), args.after.read_text(),
                           json.loads(args.analysis.read_text())), indent=2))
