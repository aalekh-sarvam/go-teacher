"""Evidence version 2 ingestion, agent slices, part merging and validator parity."""
import copy
import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT/'scripts'
sys.path.insert(0, str(SCRIPTS))
from parse_review import parse_review
from lesson_contract import brief
from teaching_facts import build_facts
from validate_lesson import validate
from slice_for_agent import build_slice
from merge_lesson_parts import merge_parts, MergeError

V1 = ROOT/'tests/sample_report_format5.md'
V2 = ROOT/'tests/sample_report_format5_v2.md'


def compact(obj):
    return len(json.dumps(obj, ensure_ascii=False, separators=(',', ':')).encode('utf-8'))


class OrchestrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parsed = parse_review(V2.read_text(encoding='utf-8'))
        cls.facts = build_facts(cls.parsed)
        cls.v5_lesson = json.loads((ROOT/'tests/sample_lesson_v5.json').read_text())
        cls.grounded = json.loads((ROOT/'tests/sample_grounded_lesson.json').read_text())

    # --- parsing -----------------------------------------------------------------------------
    def test_parser_accepts_v1_and_v2_and_rejects_v3(self):
        v1 = parse_review(V1.read_text(encoding='utf-8'))
        self.assertEqual(v1['evidence_version'], 1)
        self.assertEqual(self.parsed['evidence_version'], 2)
        self.assertEqual(self.parsed['report_format'], 5)
        text = V2.read_text(encoding='utf-8').replace('"evidence_version":2', '"evidence_version":3')
        with self.assertRaises(SystemExit) as cm:
            parse_review(text)
        self.assertIn('[1, 2]', str(cm.exception))
        self.assertIn('3', str(cm.exception))

    def test_v2_fields_pass_through_untouched(self):
        p = self.parsed
        praise = p['teaching']['praise'][0]
        self.assertEqual((praise['kind'], praise['kind_label'], praise['stones_captured']), ('Capture', 'capture', 1))
        self.assertTrue(praise['rating'] and praise['rating_source'])
        self.assertEqual(p['moves'][18]['stones_captured'], 1)
        self.assertEqual(p['student_profile']['estimate']['rank_label'], '10k')
        c7 = p['teaching']['candidates'][0]
        self.assertEqual(c7['refutation_with_colours'], ['W L3'])
        self.assertEqual(c7['better_line_with_colours'], ['B K5'])
        self.assertEqual(c7['evidence']['sequences'][1]['moves_with_colours'], ['B R14', 'W L3'])
        self.assertEqual(p['move_details']['7']['candidates'][0]['pv_with_colours'], ['B K5'])
        for key in ('student_profile', 'praise', 'colours'):
            self.assertIn(key, p['interpretation'])

    def test_brief_keeps_profile_praise_and_captures(self):
        view = brief(self.parsed)
        self.assertEqual(view['student_profile'], self.parsed['student_profile'])
        self.assertEqual(view['teaching']['praise'], self.parsed['teaching']['praise'])
        self.assertEqual([m['stones_captured'] for m in view['moves']], [m['stones_captured'] for m in self.parsed['moves']])
        self.assertNotIn('move_details', view)

    def test_parse_cli_moves_passthrough(self):
        with tempfile.TemporaryDirectory() as d:
            out, facts = Path(d)/'parsed.json', Path(d)/'facts.json'
            r = subprocess.run([sys.executable, str(SCRIPTS/'parse_review.py'), str(V2), str(out),
                                '--facts-output', str(facts), '--moves', '9'], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn('evidence v2', r.stderr)
            f = json.loads(facts.read_text())
            self.assertEqual(list(f['lessons']), ['9'])
            self.assertEqual(len(f['praise_facts']), 1)   # praise facts are built regardless of --moves

    # --- facts -------------------------------------------------------------------------------
    def test_facts_have_coloured_sequences_and_praise_facts(self):
        lesson = self.facts['lessons']['7']
        self.assertEqual(lesson['sequences_with_colours']['played_engine'], ['B R14', 'W L3'])
        self.assertEqual(lesson['sequences_with_colours']['pass'], ['B pass', 'W L3'])
        self.assertEqual(lesson['refutation_with_colours'], ['W L3'])
        self.assertEqual(lesson['better_line_with_colours'], ['B K5'])
        # fallback when the report lacks moves_with_colours (evidence v1)
        v1_facts = build_facts(parse_review(V1.read_text(encoding='utf-8')), [7])
        self.assertEqual(v1_facts['lessons']['7']['sequences_with_colours']['played_engine'], ['B R14', 'W L3'])
        self.assertEqual(v1_facts['praise_facts'], [])
        pf = self.facts['praise_facts'][0]
        self.assertEqual(pf['move'], 'O4')
        self.assertEqual(pf['kind_label'], 'capture')
        self.assertEqual(pf['stones_captured'], 1)
        self.assertEqual(pf['gap'], 2.4)
        self.assertEqual(pf['human_prob_percent'], 31)
        self.assertEqual(pf['rating'], 'a move most 8k players find first')
        self.assertTrue(pf['rating_source'])
        self.assertIn('captured 1 stone', pf['note'])

    def test_praise_fact_drops_rating_without_source(self):
        parsed = copy.deepcopy(self.parsed)
        parsed['teaching']['praise'][0]['rating_source'] = None
        pf = build_facts(parsed)['praise_facts'][0]
        self.assertIsNone(pf['rating'])
        self.assertIn('rating_omitted', pf)

    # --- slices ------------------------------------------------------------------------------
    def test_slices_are_smaller_than_parsed_and_carry_the_right_parts(self):
        full = compact(self.parsed)
        moment = build_slice(self.parsed, moment=7, facts=self.facts)
        self.assertLess(compact(moment), full)
        self.assertEqual(moment['candidate']['move_number'], 7)
        self.assertEqual(set(moment['move_details']), {'7'})          # 6 and 8 have no details in the fixture
        self.assertEqual([m['number'] for m in moment['moves']], list(range(1, 20)))   # 7-8..7+12 clamped to the game
        self.assertIn('sequences_with_colours', moment['facts'])
        self.assertNotIn('sequences', moment['facts'])
        self.assertEqual(moment['student_profile'], self.parsed['student_profile'])
        self.assertIn('colours', moment['interpretation'])

        overview = build_slice(self.parsed, role='overview')
        self.assertLess(compact(overview), full)
        self.assertEqual(len(overview['moves']), 20)
        self.assertEqual(set(overview['move_details']), {'20'})      # context move with details
        for key in ('arc', 'summary', 'openings', 'history', 'context_moves'):
            self.assertIn(key, overview)
        self.assertNotIn('candidate', overview)

        praise = build_slice(self.parsed, role='praise', facts=self.facts)
        self.assertLess(compact(praise), full)
        self.assertEqual(praise['praise'], self.parsed['teaching']['praise'])
        self.assertIn(19, [m['number'] for m in praise['moves']])
        self.assertTrue(all((m.get('stones_captured') or 0) > 0 or m.get('point_loss', 0) < 0 for m in praise['moves']))
        self.assertEqual(praise['praise_facts'][0]['move'], 'O4')

        concepts = build_slice(self.parsed, role='concepts', moments=[7, 9])
        self.assertLess(compact(concepts), full)
        self.assertEqual([t['theme'] for t in concepts['themes']], ['direction of play', 'over-defending / priority'])
        self.assertNotIn('evidence', concepts['themes'][0])

        with self.assertRaises(SystemExit):
            build_slice(self.parsed, moment=8)
        with self.assertRaises(SystemExit):
            build_slice(self.parsed, role='concepts', moments=[])

    def test_slice_cli_reports_size(self):
        with tempfile.TemporaryDirectory() as d:
            parsed = Path(d)/'parsed.json'; parsed.write_text(json.dumps(self.parsed))
            out = Path(d)/'m7.json'
            r = subprocess.run([sys.executable, str(SCRIPTS/'slice_for_agent.py'), str(parsed), '--moment', '7', '-o', str(out)],
                               capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertRegex(r.stderr, r'\d+ bytes')
            self.assertEqual(json.loads(out.read_text())['slice'], {'role': 'lesson', 'moment': 7})
            r = subprocess.run([sys.executable, str(SCRIPTS/'slice_for_agent.py'), str(parsed), '--moment', '7', '--role', 'praise', '-o', str(out)],
                               capture_output=True, text=True)
            self.assertNotEqual(r.returncode, 0)

    # --- merging -----------------------------------------------------------------------------
    def make_parts(self):
        g = copy.deepcopy(self.grounded)
        overview = {'part': 'overview', 'schema_version': 2, 'grounding_version': 1}
        for key in ('game_title', 'overall_feedback', 'game_arc'):
            overview[key] = g[key]
        lesson7 = {'part': 'lesson', 'schema_version': 2, 'grounding_version': 1,
                   'lesson': g['lessons'][0], 'fact_checks': g['fact_checks'], 'puzzles': g['puzzles']}
        lesson9 = {'part': 'lesson', 'schema_version': 2, 'grounding_version': 1,
                   'lesson': {'move_number': 9, 'title': 'Answer only urgent moves', 'concept_label': 'Priority',
                              'story': 'The actual recorded move is B R17, a local answer to W Q18.',
                              'principle': 'Ask whether the local reply is urgent before answering.',
                              'panels': {'played': 'The reply was not urgent.', 'best': 'M1 was worth more.'}},
                   'fact_checks': [{'text_path': '/lessons/0/story', 'quote': 'The actual recorded move is B R17',
                                    'fact': '/actual_moves/9', 'expected': {'color': 'B', 'move': 'R17'}}]}
        praise = {'part': 'praise', 'good_moves': [{
            'move_number': 19, 'move': 'O4',
            'explanation': 'O4 captured 1 stone and was the top choice; the next best searched move was 2.4 points worse.',
            'rating': 'a move most 8k players find first',
            'rating_source': 'human-style network: weakest tested profile whose most common move is this move'}]}
        res = {'title': "Sensei's Library: Direction of play", 'url': 'https://senseis.xmp.net/?DirectionOfPlay'}
        concepts_a = {'part': 'concepts', 'concepts_learned': [
            {'term': 'Direction of play', 'description': 'Which way stones want to develop.', 'resources': [res, dict(res)]}]}
        concepts_b = {'part': 'concepts', 'concepts_learned': [
            {'term': 'direction of play', 'description': 'duplicate by term', 'resources': []},
            {'term': 'Urgent before big', 'description': 'Settle weak groups first.',
             'resources': [{'title': 'Go Magic', 'url': 'https://gomagic.org/lessons/three-stages-of-the-game/'}]}]}
        # Deliberately out of move order: lesson 9 before lesson 7.
        return [('overview.json', overview), ('lesson9.json', lesson9), ('lesson7.json', lesson7),
                ('praise.json', praise), ('concepts_a.json', concepts_a), ('concepts_b.json', concepts_b)]

    def test_merge_orders_lessons_renumbers_fact_checks_and_validates(self):
        merged = merge_parts(self.make_parts())
        self.assertEqual([l['move_number'] for l in merged['lessons']], [7, 9])
        paths = [fc['text_path'] for fc in merged['fact_checks']]
        self.assertEqual(paths, ['/lessons/0/story', '/lessons/1/story'])
        self.assertEqual(merged['fact_checks'][1]['fact'], '/actual_moves/9')
        self.assertEqual(len(merged['puzzles']), 1)
        self.assertEqual([c['term'] for c in merged['concepts_learned']], ['Direction of play', 'Urgent before big'])
        self.assertEqual(len(merged['concepts_learned'][0]['resources']), 1)
        self.assertEqual((merged['schema_version'], merged['grounding_version']), (2, 1))
        errors, warnings = validate(self.parsed, merged)
        self.assertEqual(errors, [])

    def test_merge_rejects_disagreeing_versions_and_bad_parts(self):
        parts = self.make_parts()
        parts[1][1]['schema_version'] = 1
        with self.assertRaises(MergeError):
            merge_parts(parts)
        parts = self.make_parts()
        parts.append(('overview2.json', dict(parts[0][1])))
        with self.assertRaises(MergeError):
            merge_parts(parts)
        parts = self.make_parts()
        parts[1][1]['fact_checks'][0]['text_path'] = '/lessons/3/story'
        with self.assertRaises(MergeError):
            merge_parts(parts)

    def test_merge_cli_runs_validator_and_fails_on_problems(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            (d/'parsed.json').write_text(json.dumps(self.parsed))
            names = []
            for name, part in self.make_parts():
                (d/name).write_text(json.dumps(part)); names.append(str(d/name))
            cmd = [sys.executable, str(SCRIPTS/'merge_lesson_parts.py'), str(d/'parsed.json'), *names, '-o', str(d/'lesson.json')]
            r = subprocess.run(cmd, capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            self.assertIn('0 problem(s)', r.stdout)
            merged = json.loads((d/'lesson.json').read_text())
            self.assertEqual(merged['fact_checks'][1]['text_path'], '/lessons/1/story')
            # A rating without its source must make the merge exit non-zero and name the problem.
            praise = json.loads((d/'praise.json').read_text())
            del praise['good_moves'][0]['rating_source']
            (d/'praise.json').write_text(json.dumps(praise))
            r = subprocess.run(cmd, capture_output=True, text=True)
            self.assertEqual(r.returncode, 1)
            self.assertIn('rating requires rating_source', r.stdout)
            self.assertTrue((d/'lesson.json').exists())   # written before validation so the author can inspect it

    # --- validator parity --------------------------------------------------------------------
    def test_validator_catches_missing_puzzle_title_and_rating_without_source(self):
        lesson = copy.deepcopy(self.v5_lesson)
        lesson['puzzles'][0].pop('title')
        lesson['good_moves'] = [{'move_number': 19, 'move': 'O4', 'explanation': 'A clean capture.',
                                 'rating': 'a move most 8k players find first'}]
        errors, _ = validate(self.parsed, lesson)
        self.assertTrue(any(e == 'puzzle 1: title missing' for e in errors), errors)
        self.assertTrue(any('rating requires rating_source' in e for e in errors), errors)
        lesson['good_moves'][0]['rating_source'] = self.parsed['teaching']['praise'][0]['rating_source']
        lesson['puzzles'][0]['title'] = 'Restored'
        self.assertEqual(validate(self.parsed, lesson)[0], [])

    def test_validator_checks_every_generator_dereference(self):
        lesson = copy.deepcopy(self.v5_lesson)
        lesson['puzzles'][0].pop('hint'); lesson['puzzles'][0].pop('concept_label'); lesson['puzzles'][0]['player_to_move'] = 'Black'
        lesson['good_moves'] = [{'move_number': 20, 'move': 'R7', 'explanation': 'opponent move'},
                                {'move_number': 19, 'move': 'Q4', 'explanation': 'wrong coordinate'},
                                {'move_number': 17, 'move': 'R4'}]
        lesson['concepts_learned'] = [{'term': 'Liberty'}, {'term': 'Atari', 'description': 'One liberty left.',
                                                             'resources': [{'title': 'no url'}, {'url': 'ftp://x', 'title': 'bad scheme'}]}]
        errors, _ = validate(self.parsed, lesson)
        for fragment in ('puzzle 1: hint missing', 'puzzle 1: concept_label missing', 'player_to_move must be',
                         'good move 1: move 20 was played by the opponent', "good move 2: move 'Q4' is not the recorded move O4",
                         'good move 3: explanation missing', 'concept 1: description missing',
                         'concept 2: resource 1 needs title and url', 'concept 2: resource 2 url must be http(s)'):
            self.assertTrue(any(fragment in e for e in errors), (fragment, errors))

    def test_validator_warns_when_theme_differs_from_report(self):
        lesson = copy.deepcopy(self.v5_lesson)
        lesson['lessons'][0]['theme'] = 'reading / tactics'
        errors, warnings = validate(self.parsed, lesson)
        self.assertEqual(errors, [])
        self.assertTrue(any("theme 'reading / tactics' differs" in w for w in warnings), warnings)
        lesson['lessons'][0]['theme'] = 'direction of play'
        self.assertFalse(any('theme' in w for w in validate(self.parsed, lesson)[1]))

    def test_praise_numbers_must_match_the_report(self):
        lesson = copy.deepcopy(self.v5_lesson)
        rating = {'rating': self.parsed['teaching']['praise'][0]['rating'], 'rating_source': self.parsed['teaching']['praise'][0]['rating_source']}
        lesson['good_moves'] = [dict(move_number=19, move='O4', **rating,
                                     explanation='O4 captured 1 stone; the next best searched move C19 was 2.4 points worse and about 31% of players at your level find it.')]
        self.assertEqual(validate(self.parsed, lesson)[0], [])
        lesson['good_moves'][0]['explanation'] = 'O4 captured 3 stones, gaining 5 points; 60% of players find it.'
        errors, _ = validate(self.parsed, lesson)
        self.assertEqual(len([e for e in errors if e.startswith('praise 1 (move 19)')]), 3, errors)
        # A move without a praise entry is not number-checked (nothing to check against).
        lesson['good_moves'] = [{'move_number': 17, 'move': 'R4', 'explanation': 'Worth 3 points.'}]
        self.assertFalse(any(e.startswith('praise') for e in validate(self.parsed, lesson)[0]))


if __name__ == '__main__':
    unittest.main()
