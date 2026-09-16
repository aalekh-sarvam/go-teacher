#!/usr/bin/env python3
"""Assemble lesson.json from separately authored parts, then validate it.

Usage: merge_lesson_parts.py parsed.json part1.json part2.json ... -o lesson.json

Each part is a JSON object with a top-level "part":
  overview  -> game_title, players, result, overall_feedback, game_arc, progress,
               overview_anchor_move, optional fact_checks whose text_path is not under /lessons/
  lesson    -> one "lesson" object, optional "fact_checks" (text_path written as /lessons/0/...),
               optional "puzzles"
  praise    -> good_moves
  concepts  -> concepts_learned
Every part may carry schema_version / grounding_version; they must agree across parts.

Lessons are ordered by move number and fact_checks are renumbered from /lessons/0/ to the
lesson's final index. Puzzles are concatenated; concepts are deduplicated by term and their
resources by url. The merged file is written first, then validate_lesson.validate runs and
the process exits non-zero if it reports problems.
"""
import argparse
import json
import sys

from validate_lesson import validate

OVERVIEW_KEYS = ('game_title', 'players', 'result', 'overall_feedback', 'game_arc', 'progress', 'overview_anchor_move')
VERSION_KEYS = ('schema_version', 'grounding_version')
PARTS = ('overview', 'lesson', 'praise', 'concepts')


class MergeError(Exception):
    pass


def load_parts(paths):
    parts = []
    for path in paths:
        with open(path, encoding='utf-8') as f:
            data = json.load(f)
        if not isinstance(data, dict) or data.get('part') not in PARTS:
            raise MergeError(f'{path}: top-level "part" must be one of {list(PARTS)}')
        parts.append((path, data))
    return parts


def merge_parts(parts):
    """parts: list of (name, dict). Returns the merged lesson dict; raises MergeError on structure problems."""
    versions = {}
    for name, data in parts:
        for key in VERSION_KEYS:
            if key in data:
                if key in versions and versions[key][0] != data[key]:
                    raise MergeError(f'{key} disagrees: {versions[key][1]} has {versions[key][0]!r}, {name} has {data[key]!r}')
                versions.setdefault(key, (data[key], name))

    overviews = [(n, d) for n, d in parts if d['part'] == 'overview']
    if len(overviews) != 1:
        raise MergeError(f'exactly one overview part is required, found {len(overviews)}')
    merged = {}
    for key in VERSION_KEYS:
        if key in versions:
            merged[key] = versions[key][0]
    _, overview = overviews[0]
    for key in OVERVIEW_KEYS:
        if key in overview:
            merged[key] = overview[key]
    fact_checks = []
    for i, fc in enumerate(overview.get('fact_checks') or []):
        path = fc.get('text_path', '')
        if not isinstance(path, str) or path.startswith('/lessons/'):
            raise MergeError(f'{overviews[0][0]}: fact check {i+1} text_path must point at overview prose, not /lessons/')
        fact_checks.append(fc)

    lesson_parts = []
    for name, data in parts:
        if data['part'] != 'lesson':
            continue
        lesson = data.get('lesson')
        if not isinstance(lesson, dict) or not isinstance(lesson.get('move_number'), int):
            raise MergeError(f'{name}: lesson part needs a "lesson" object with an integer move_number')
        lesson_parts.append((name, data))
    numbers = [d['lesson']['move_number'] for _, d in lesson_parts]
    if len(set(numbers)) != len(numbers):
        raise MergeError(f'duplicate lesson move numbers: {sorted(n for n in set(numbers) if numbers.count(n) > 1)}')
    lesson_parts.sort(key=lambda nd: nd[1]['lesson']['move_number'])

    lessons, puzzles = [], []
    for index, (name, data) in enumerate(lesson_parts):
        lessons.append(data['lesson'])
        for i, fc in enumerate(data.get('fact_checks') or []):
            path = fc.get('text_path', '')
            if not isinstance(path, str) or not path.startswith('/lessons/0/'):
                raise MergeError(f'{name}: fact check {i+1} text_path must start with /lessons/0/ (got {path!r})')
            fact_checks.append(dict(fc, text_path=f'/lessons/{index}/' + path[len('/lessons/0/'):]))
        for p in data.get('puzzles') or []:
            puzzles.append(p)
    merged['lessons'] = lessons
    merged['puzzles'] = puzzles

    good_moves = []
    for name, data in parts:
        if data['part'] == 'praise':
            gm = data.get('good_moves')
            if not isinstance(gm, list):
                raise MergeError(f'{name}: praise part needs a "good_moves" list')
            good_moves.extend(gm)
    merged['good_moves'] = good_moves

    concepts, seen_terms = [], set()
    for name, data in parts:
        if data['part'] != 'concepts':
            continue
        entries = data.get('concepts_learned')
        if not isinstance(entries, list):
            raise MergeError(f'{name}: concepts part needs a "concepts_learned" list')
        for c in entries:
            term = (c.get('term') or '').strip().lower() if isinstance(c, dict) else ''
            if term and term in seen_terms:
                continue
            seen_terms.add(term)
            if isinstance(c, dict) and isinstance(c.get('resources'), list):
                urls, resources = set(), []
                for r in c['resources']:
                    url = r.get('url') if isinstance(r, dict) else None
                    if url and url in urls:
                        continue
                    if url:
                        urls.add(url)
                    resources.append(r)
                c = dict(c, resources=resources)
            concepts.append(c)
    merged['concepts_learned'] = concepts
    if fact_checks or merged.get('grounding_version') == 1:
        merged['fact_checks'] = fact_checks
    return merged


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('parsed')
    ap.add_argument('parts', nargs='+')
    ap.add_argument('-o', '--output', required=True)
    args = ap.parse_args()
    with open(args.parsed, encoding='utf-8') as f:
        parsed = json.load(f)
    try:
        merged = merge_parts(load_parts(args.parts))
    except MergeError as e:
        print('PROBLEM:', e)
        sys.exit(2)
    with open(args.output, 'w', encoding='utf-8') as f:
        json.dump(merged, f, indent=1, ensure_ascii=False)
    errors, warnings = validate(parsed, merged)
    for w in warnings:
        print('warning:', w)
    for e in errors:
        print('PROBLEM:', e)
    print(f'{args.output}: {len(merged["lessons"])} lesson(s), {len(merged["puzzles"])} puzzle(s), '
          f'{len(merged["good_moves"])} good move(s), {len(merged["concepts_learned"])} concept(s); '
          f'{len(errors)} problem(s), {len(warnings)} warning(s)')
    sys.exit(bool(errors))


if __name__ == '__main__':
    main()
