#!/usr/bin/env python3
"""Cut parsed.json down to the slice one authoring step needs (target 10-25 KB per slice).

Usage:
  slice_for_agent.py parsed.json --moment N [--facts facts.json] -o slice.json
      One teaching moment: the candidate with its evidence, move_details for N-1/N/N+1,
      facts.lessons[N] when facts.json is given, moves rows N-8..N+12.
  slice_for_agent.py parsed.json --role overview -o slice.json
      Game-level context: arc, summary, openings, history, every moves row, context moves.
  slice_for_agent.py parsed.json --role praise [--facts facts.json] -o slice.json
      teaching.praise plus the moves rows with stones_captured > 0 or point_loss < 0.
  slice_for_agent.py parsed.json --role concepts --moments N [N ...] -o slice.json
      The themes of the chosen candidates, for the concepts/resources section.

Every slice carries game_info and student_profile. The byte size goes to stderr.
Slices are reading views for the writing agent; validation and generation still use parsed.json.
"""
import argparse
import json
import sys
from copy import deepcopy

TARGET_MIN, TARGET_MAX = 10_000, 25_000
THEME_KEYS = ('move_number', 'player', 'played_move', 'preferred_move', 'point_loss', 'theme', 'phase',
              'category', 'captured_at', 'loss_region', 'better_move_is', 'difficulty')


def _row_number(entry):
    """context_moves entries are ints in evidence v1/v2; tolerate {'move_number': n} / {'number': n}."""
    if isinstance(entry, bool):
        return None
    if isinstance(entry, int):
        return entry
    if isinstance(entry, dict):
        n = entry.get('move_number', entry.get('number'))
        return n if isinstance(n, int) else None
    return None


def context_move_numbers(parsed):
    cm = parsed.get('context_moves') or {}
    out = set()
    for key in ('opponent_mistakes', 'checkpoints'):
        for e in cm.get(key) or []:
            n = _row_number(e)
            if n is not None:
                out.add(n)
    n = _row_number(cm.get('last_move'))
    if n is not None:
        out.add(n)
    return sorted(out)


def common(parsed):
    out = {'game_info': deepcopy(parsed['game_info'])}
    if 'student_profile' in parsed:
        out['student_profile'] = deepcopy(parsed['student_profile'])
    return out


def details_for(parsed, numbers):
    details = parsed.get('move_details') or {}
    return {str(n): deepcopy(details[str(n)]) for n in numbers if str(n) in details}


def candidate(parsed, n):
    for c in parsed.get('teaching', {}).get('candidates', []):
        if c.get('move_number') == n:
            return c
    return None


def slice_moment(parsed, n, facts=None):
    c = candidate(parsed, n)
    if c is None:
        available = [x.get('move_number') for x in parsed.get('teaching', {}).get('candidates', [])]
        raise SystemExit(f'move {n} is not a teaching candidate; candidates are {available}')
    moves = parsed.get('moves') or []
    out = common(parsed)
    out['profiles'] = deepcopy(parsed.get('profiles'))
    out['arc'] = {'phases': deepcopy((parsed.get('arc') or {}).get('phases', []))}
    out['candidate'] = deepcopy(c)
    out['move_details'] = details_for(parsed, (n-1, n, n+1))
    if facts is not None:
        lesson_facts = (facts.get('lessons') or {}).get(str(n))
        if lesson_facts is None:
            raise SystemExit(f'facts.json has no lessons[{n}]; rebuild facts with --moves {n} or without --moves')
        lesson_facts = deepcopy(lesson_facts)
        # Full plies live in candidate.evidence.sequences; sequences_with_colours keeps the quotable form.
        lesson_facts.pop('sequences', None)
        lesson_facts['sequences_note'] = 'full sequences are in candidate.evidence.sequences; quote sequences_with_colours'
        out['facts'] = lesson_facts
    out['moves'] = deepcopy([m for m in moves if n-8 <= m['number'] <= n+12])
    out['interpretation'] = deepcopy(parsed.get('interpretation', {}))
    out['slice'] = {'role': 'lesson', 'moment': n}
    return out


def slice_overview(parsed):
    out = common(parsed)
    for key in ('arc', 'summary', 'openings', 'history'):
        if key in parsed:
            out[key] = deepcopy(parsed[key])
    out['moves'] = deepcopy(parsed.get('moves') or [])
    out['context_moves'] = deepcopy(parsed.get('context_moves'))
    out['move_details'] = details_for(parsed, context_move_numbers(parsed))
    out['interpretation'] = deepcopy(parsed.get('interpretation', {}))
    out['slice'] = {'role': 'overview'}
    return out


def slice_praise(parsed, facts=None):
    out = common(parsed)
    out['praise'] = deepcopy(parsed.get('teaching', {}).get('praise', []))
    rows = [m for m in (parsed.get('moves') or [])
            if (m.get('stones_captured') or 0) > 0 or (isinstance(m.get('point_loss'), (int, float)) and m['point_loss'] < 0)]
    out['moves'] = deepcopy(rows)
    out['move_details'] = details_for(parsed, [m['number'] for m in rows])
    if facts is not None and 'praise_facts' in facts:
        out['praise_facts'] = deepcopy(facts['praise_facts'])
    out['interpretation'] = {k: v for k, v in (parsed.get('interpretation') or {}).items() if k in ('praise', 'colours', 'moves', 'student_profile')}
    out['slice'] = {'role': 'praise'}
    return out


def slice_concepts(parsed, moments):
    out = common(parsed)
    themes = []
    for n in moments:
        c = candidate(parsed, n)
        if c is None:
            raise SystemExit(f'move {n} is not a teaching candidate')
        themes.append({k: deepcopy(c[k]) for k in THEME_KEYS if k in c})
    out['themes'] = themes
    out['slice'] = {'role': 'concepts', 'moments': list(moments)}
    return out


def build_slice(parsed, role=None, moment=None, moments=None, facts=None):
    if moment is not None:
        return slice_moment(parsed, moment, facts)
    if role == 'overview':
        return slice_overview(parsed)
    if role == 'praise':
        return slice_praise(parsed, facts)
    if role == 'concepts':
        if not moments:
            raise SystemExit('--role concepts needs --moments N [N ...]')
        return slice_concepts(parsed, moments)
    raise SystemExit('give --moment N or --role overview|praise|concepts')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('parsed')
    ap.add_argument('--moment', type=int, metavar='N', help='teaching candidate move number for one lesson slice')
    ap.add_argument('--role', choices=('overview', 'praise', 'concepts'))
    ap.add_argument('--moments', type=int, nargs='+', metavar='N', help='candidate move numbers for --role concepts')
    ap.add_argument('--facts', help='facts.json from parse_review.py --facts-output (adds facts.lessons[N] / praise_facts)')
    ap.add_argument('-o', '--output', required=True)
    args = ap.parse_args()
    if args.moment is not None and args.role:
        raise SystemExit('use either --moment or --role, not both')
    with open(args.parsed, encoding='utf-8') as f:
        parsed = json.load(f)
    if parsed.get('reading_view_only'):
        raise SystemExit('Slice from the complete parsed.json, not brief.json')
    facts = None
    if args.facts:
        with open(args.facts, encoding='utf-8') as f:
            facts = json.load(f)
    out = build_slice(parsed, role=args.role, moment=args.moment, moments=args.moments, facts=facts)
    text = json.dumps(out, ensure_ascii=False, separators=(',', ':'))   # compact: the reader is a model
    with open(args.output, 'w', encoding='utf-8') as f:
        f.write(text)
    size = len(text.encode('utf-8'))
    note = '' if TARGET_MIN <= size <= TARGET_MAX else f' (outside the {TARGET_MIN//1000}-{TARGET_MAX//1000} KB target)'
    print(f'{args.output}: {size} bytes{note}', file=sys.stderr)


if __name__ == '__main__':
    main()
