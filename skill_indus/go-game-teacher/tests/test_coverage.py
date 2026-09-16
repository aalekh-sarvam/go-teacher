"""Deep-coverage fields: pass-through, derived summary, legacy interpretation, brief, facts and validator checks."""
import copy
import json
import subprocess
import sys
import unittest
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT/'scripts'
sys.path.insert(0, str(SCRIPTS))
from parse_review import parse_review, coverage_summary
from lesson_contract import brief
from teaching_facts import build_facts, expand_facts
from validate_lesson import validate, check_coverage_claims

V1 = ROOT/'tests/sample_report_format5.md'
V2 = ROOT/'tests/sample_report_format5_v2.md'
V4 = ROOT/'tests/sample_report_format4.md'


def by_move(parsed):
    return {c['move_number']: c for c in coverage_summary(parsed)}


class CoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.v2 = parse_review(V2.read_text(encoding='utf-8'))
        cls.v1 = parse_review(V1.read_text(encoding='utf-8'))
        cls.lesson = json.loads((ROOT/'tests/sample_lesson_v5.json').read_text())
        cls.grounded = json.loads((ROOT/'tests/sample_grounded_lesson.json').read_text())

    # --- parsing and derived summary --------------------------------------------------------
    def test_v2_fields_pass_through(self):
        p = self.v2
        self.assertEqual(p['search_coverage']['version'], 1)
        self.assertEqual((p['search_coverage']['final_candidates'], p['search_coverage']['verified_candidates']), (3, 2))
        self.assertIn('coverage', p['interpretation'])
        c7, c9, c11 = p['teaching']['candidates']
        self.assertEqual(c7['evaluation_coverage']['before'], {'turn': 6, 'visits': 20, 'requested_visits': 20, 'purpose': 'deep', 'status': 'complete'})
        self.assertEqual(c7['investigation_coverage'], {'status': 'available'})
        self.assertEqual(c9['investigation_coverage'], {'status': 'not_selected', 'reason': 'outside_featured_budget'})
        self.assertEqual(list(c9['evidence']['unavailable']), ['teaching_investigations'])
        self.assertEqual(list(c11['evidence']['unavailable']), ['deeper_search'])   # legacy key kept as written

    def test_v2_summary_complete_with_and_without_investigations(self):
        cov = by_move(self.v2)
        self.assertEqual(cov[7], {'move_number': 7, 'evaluation_status': 'complete', 'before_visits': 20, 'after_visits': 20,
                                  'investigation_status': 'available', 'legacy': False})
        self.assertEqual(cov[9], {'move_number': 9, 'evaluation_status': 'complete', 'before_visits': 20, 'after_visits': 20,
                                  'investigation_status': 'not_selected', 'legacy': False})
        # a candidate without the fields in an otherwise-new report is unknown, and the legacy key only means not_selected
        self.assertEqual(cov[11], {'move_number': 11, 'evaluation_status': 'unknown', 'before_visits': None, 'after_visits': None,
                                   'investigation_status': 'not_selected', 'legacy': True})

    def test_legacy_v1_and_format4_are_unknown_never_incomplete(self):
        for parsed in (self.v1, parse_review(V4.read_text(encoding='utf-8'))):
            cov = coverage_summary(parsed)
            self.assertTrue(cov)
            self.assertTrue(all(c['evaluation_status'] == 'unknown' and c['legacy'] for c in cov))
            self.assertTrue(all(c['before_visits'] is None and c['after_visits'] is None for c in cov))
            self.assertNotIn('incomplete', {c['evaluation_status'] for c in cov})
        v1 = by_move(self.v1)
        self.assertEqual(v1[7]['investigation_status'], 'unknown')        # sequences exist but nothing was recorded
        self.assertEqual(v1[9]['investigation_status'], 'not_selected')   # legacy deeper_search: investigations only
        self.assertEqual(v1[9]['evaluation_status'], 'unknown')           # ...never a statement about deep evaluation

    def test_unknown_statuses_and_coverage_versions_degrade_to_unknown(self):
        parsed = copy.deepcopy(self.v2)
        parsed['teaching']['candidates'][0]['evaluation_coverage']['status'] = 'verified-ish'
        self.assertEqual(by_move(parsed)[7]['evaluation_status'], 'unknown')
        parsed = copy.deepcopy(self.v2)
        parsed['search_coverage']['version'] = 99
        cov = by_move(parsed)
        self.assertEqual((cov[7]['evaluation_status'], cov[7]['before_visits']), ('unknown', None))
        self.assertEqual(cov[9]['investigation_status'], 'not_selected')   # the unavailable key still says that much

    def test_parse_cli_reports_coverage(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            r = subprocess.run([sys.executable, str(SCRIPTS/'parse_review.py'), str(V2), str(Path(d)/'p.json')], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn('deep evaluation complete: 2/3', r.stderr)
            r = subprocess.run([sys.executable, str(SCRIPTS/'parse_review.py'), str(V1), str(Path(d)/'p.json')], capture_output=True, text=True)
            self.assertIn('coverage not recorded', r.stderr)

    # --- brief and facts ---------------------------------------------------------------------
    def test_brief_keeps_coverage_fields(self):
        view = brief(self.v2)
        self.assertEqual(view['search_coverage'], self.v2['search_coverage'])
        for original, kept in zip(self.v2['teaching']['candidates'], view['teaching']['candidates']):
            self.assertEqual(kept.get('evaluation_coverage'), original.get('evaluation_coverage'))
            self.assertEqual(kept.get('investigation_coverage'), original.get('investigation_coverage'))
            self.assertEqual(kept['evidence'].get('unavailable'), original['evidence'].get('unavailable'))
        self.assertNotIn('search_coverage', brief(self.v1))

    def test_facts_expose_quotable_coverage(self):
        facts = build_facts(self.v2)
        self.assertEqual(facts['search_coverage'], self.v2['search_coverage'])
        self.assertIn('verified at 20 visits on both sides', facts['coverage']['7']['text'])
        self.assertIn('investigations (human/local/what-if sequences) are available', facts['coverage']['7']['text'])
        self.assertIn('No teaching investigations were run (not selected)', facts['coverage']['9']['text'])
        self.assertEqual(facts['lessons']['7']['coverage']['evaluation_status'], 'complete')
        self.assertIn('Coverage not recorded', facts['coverage']['11']['text'])
        self.assertNotIn('verified', facts['coverage']['11']['text'].split('do not call')[0])
        expanded = expand_facts({'story': 'Note: {{fact:/lessons/7/coverage/text}}'}, facts)
        self.assertIn('verified at 20 visits', expanded['story'])
        v1_facts = build_facts(self.v1)
        self.assertTrue(all(e['legacy'] and 'Coverage not recorded' in e['text'] for e in v1_facts['coverage'].values()))
        self.assertIsNone(v1_facts['search_coverage'])

    # --- validator ---------------------------------------------------------------------------
    def _incomplete_parsed(self):
        parsed = copy.deepcopy(self.v2)
        c7 = parsed['teaching']['candidates'][0]
        c7['evaluation_coverage']['status'] = 'incomplete'
        c7['evaluation_coverage']['after'].update(status='pending', visits=10)
        return parsed

    def test_validator_flags_verified_claim_on_incomplete_candidate(self):
        lesson = copy.deepcopy(self.lesson)
        lesson['lessons'][0]['story'] = 'KataGo deeply verified this position before and after B R14.'
        errors, warnings = validate(self._incomplete_parsed(), lesson)
        self.assertTrue(any('deeply verified' in e and "'incomplete'" in e for e in errors), errors)
        self.assertTrue(any('primary lesson uses move 7 whose deep evaluation is incomplete' in w for w in warnings), warnings)
        # the same claim on a complete candidate is fine
        errors, warnings = validate(self.v2, lesson)
        self.assertEqual([e for e in errors if 'evaluation_coverage' in e], [])
        self.assertEqual([w for w in warnings if 'incomplete' in w], [])

    def test_verified_claim_on_unknown_coverage_is_a_problem_but_negation_is_not(self):
        lesson = copy.deepcopy(self.lesson)
        lesson['lessons'][0]['panels']['position'] = 'This is a deep evaluation of the position.'
        errors, _ = validate(self.v1, lesson)                       # legacy report: coverage unknown
        self.assertTrue(any('deep evaluation' in e and "'unknown'" in e for e in errors), errors)
        lesson['lessons'][0]['panels']['position'] = 'This position was not verified at deeper visits; the base review is the evidence.'
        errors, _ = validate(self.v1, lesson)
        self.assertEqual([e for e in errors if 'evaluation_coverage' in e], [])

    def test_validator_warns_on_invented_human_sequence_for_not_selected_candidate(self):
        lesson = copy.deepcopy(self.lesson)
        l = lesson['lessons'][0]
        l['move_number'] = 9
        l['panels'] = {'position': 'Look at the upper right first.',
                       'played': 'White answers B R17 with W N18. The most likely human reply at your level would be different.',
                       'best': 'B M1 is bigger.', 'alternatives': 'Each branch was evaluated separately.'}
        errors, warnings = check_coverage_claims(self.v2, lesson)
        self.assertEqual(errors, [])
        self.assertTrue(any('human reply' in w and "'not_selected'" in w and 'do not exist' in w for w in warnings), warnings)
        l['panels']['local'] = 'A restricted reading shows the group lives.'
        _, warnings = check_coverage_claims(self.v2, lesson)
        self.assertTrue(any('panels.local is written' in w and 'omit the panel' in w for w in warnings), warnings)
        # the whole validator keeps these as warnings, not problems
        errors, warnings = validate(self.v2, lesson)
        self.assertEqual([e for e in errors if 'investigation_coverage' in e], [])
        self.assertTrue(any('investigation_coverage' in w for w in warnings))
        # the same prose for move 7 (investigations available) raises nothing
        l['move_number'] = 7
        self.assertEqual(check_coverage_claims(self.v2, lesson), ([], []))

    def test_old_lessons_without_coverage_stay_valid(self):
        errors, warnings = validate(self.v1, self.lesson)
        self.assertEqual(errors, [])
        self.assertEqual([w for w in warnings if 'coverage' in w], [])
        errors, warnings = validate(self.v2, self.grounded)
        self.assertEqual(errors, [])
        self.assertEqual([w for w in warnings if 'coverage' in w], [])
        self.assertEqual(check_coverage_claims(self.v1, {'lessons': [{'move_number': 9, 'story': 'no claim here'}]}), ([], []))


if __name__ == '__main__':
    unittest.main()
