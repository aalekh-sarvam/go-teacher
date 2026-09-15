"""Acceptance cases from the before/after lesson comparison, with anonymised source evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from teaching_facts import build_facts, expand_facts, group_fact
from check_grounding import check_grounding, check_puzzle
from puzzle_reading import capture_within, check_capture_choice
from go_rules import Position
from validate_lesson import validate
from verify_puzzle_engine import fingerprint, check_artifact
from generate_lesson import generate_html
from lesson_contract import hydrate

class GroundingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parsed=json.loads((ROOT/'tests/grounding_case.json').read_text())
        cls.facts=build_facts(cls.parsed)

    def test_actual_game_and_alternating_variation_are_distinct(self):
        f=self.facts
        self.assertEqual(f['actual_moves']['6'],{'color':'W','move':'G6'})
        seq=f['lessons']['5']['sequences']['played_engine']['plies']
        self.assertEqual([(x['color'],x['move']) for x in seq[:4]], [('B','F7'),('W','E4'),('B','D4'),('W','D5')])

    def test_white_defender_and_initial_pass(self):
        trial=self.facts['lessons']['17']['local_trials']['defender_first']
        self.assertEqual((trial['defender'],trial['first_local_player']),('W','W'))
        self.assertEqual(trial['ownership_for_defender'],0.979)
        self.assertTrue(trial['artificial_initial_pass'])
        seq=self.facts['lessons']['17']['sequences']['defender_first']['plies']
        self.assertEqual(seq[0]['color'],'B');self.assertEqual(seq[0]['move'],'pass')
        self.assertEqual(seq[1]['color'],'W')

    def test_policy_is_attached_to_played_move_and_comparison_sign_is_correct(self):
        f=self.facts['lessons']
        self.assertEqual(f['5']['policy']['target_vs_student'],'higher')
        self.assertEqual(f['23']['policy']['target']['move'],'F9')
        self.assertEqual(f['23']['rollouts']['student']['favours_start'],'played')
        self.assertAlmostEqual(f['23']['rollouts']['student']['difference_for_student'],-10.950)

    def test_geometry_does_not_turn_ownership_prediction_into_local_kill(self):
        c=self.facts['lessons']['23']
        self.assertEqual(c['coordinates']['D8']['row_from_top'],2)
        self.assertTrue(c['coordinates']['D8']['location'].startswith('upper'))
        group=c['groups']['F2']
        for stage in ('before','after_best','after_played'):
            self.assertEqual(group[stage]['liberties'],['G1'])

    def test_current_stats_exclude_previous_analysis_snapshot(self):
        f=self.facts
        self.assertEqual(f['current']['accuracy']['top1_percent'],29.412)
        self.assertEqual([p['student_mean_loss'] for p in f['current']['phases']],[0.656,0.503,0.257])
        self.assertEqual(len(f['history']['prior_games']),1)
        self.assertEqual(len(f['history']['possible_same_game_snapshots']),1)
        self.assertEqual(f['current']['progress_rows'][2]['this_game'],'0.503')
        self.assertEqual(f['current']['progress_rows'][2]['recent_average'],'8.304')
        self.assertEqual(len(self.parsed['history']),2)  # Never delete source evidence.

    def draft(self):
        return {'grounding_version':1,'lessons':[{'panels':{'local':'White is the defender.'}}],
                'fact_checks':[{'text_path':'/lessons/0/panels/local','quote':'White is the defender.',
                  'fact':'/lessons/17/local_trials/defender_first/defender','expected':'W'}]}

    def test_wrong_attribution_in_claim_audit_is_rejected(self):
        d=self.draft();self.assertEqual(check_grounding(self.parsed,d),[])
        d['fact_checks'][0]['expected']='B'
        self.assertTrue(check_grounding(self.parsed,d))
        d=self.draft();d['fact_checks'][0]['fact']='/lessons/23/rollouts/student/favours_start';d['fact_checks'][0]['expected']='best'
        self.assertTrue(check_grounding(self.parsed,d))

    def test_checks_must_match_real_prose_and_tokens_cannot_invent_facts(self):
        d=self.draft();d['fact_checks'][0]['quote']='not written'
        self.assertTrue(check_grounding(self.parsed,d))
        text=expand_facts('{{fact:/lessons/23/rollouts/student/text}}',self.facts)
        self.assertIn('played start by 10.95',text)
        with self.assertRaises(KeyError):expand_facts('{{fact:/made_up}}',self.facts)

    def test_false_escape_quiz_is_refuted_by_full_followup(self):
        puzzle={'board_size':9,'black_stones':['C2','D3','E2','E1'],'white_stones':['C1','D1','D2'],'player_to_move':'B'}
        goal={'kind':'capture_within','attacker':'B','target':'D1','plies':5}
        # Both the accepted B1 and supposedly wrong G5 force capture within the claimed horizon.
        self.assertTrue(check_capture_choice(puzzle,'B1',goal)[0])
        self.assertTrue(check_capture_choice(puzzle,'G5',goal)[0])
        p=Position(9,black=puzzle['black_stones'],white=puzzle['white_stones'])
        for mv in ['G5','B1','B2','A1','A2']:p.play(mv)
        self.assertEqual(sum(c=='W' for c in p.board.values()),0)

    def test_budget_exhaustion_is_unknown_and_more_liberties_not_life(self):
        p=Position(9,black=['E5'],white=['D5','F5','E6'],to_move='B')
        self.assertIsNone(capture_within(p,'E5','W',5,node_budget=0)[0])
        p.play('E4')
        self.assertEqual(group_fact(p,'E5')['liberty_count'],3)

    def test_wrong_liberty_count_is_rejected(self):
        puzzle={'board_size':9,'black_stones':['D4','D5','E4','E5'],'white_stones':['F5','F6','G4'],
                'board_checks':[{'anchor':'D4','expected':{'liberty_count':8}}]}
        self.assertTrue(any('liberty_count is 7' in e for e in check_puzzle(puzzle)))

    def test_engine_certificate_must_match_rules_position_and_scores(self):
        puzzle={'correct_moves':['D4'],'wrong_moves':[{'move':'E4'}]}
        data={'puzzle_fingerprint':fingerprint(puzzle),'score_perspective':'Black',
              'responses':{'root':{'moveInfos':[{'move':'D4'}]},
                'D4':{'rootInfo':{'visits':100,'scoreLead':5}},'E4':{'rootInfo':{'visits':100,'scoreLead':4.8}}}}
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'check.json';path.write_text(json.dumps(data))
            self.assertTrue(any('does not clearly distinguish' in e for e in check_artifact(puzzle,path)))
            changed=copy.deepcopy(puzzle);changed['komi']=0.5
            self.assertTrue(any('another position' in e for e in check_artifact(changed,path)))

    def test_grounded_draft_validates_and_renders_end_to_end(self):
        from parse_review import parse_review
        p=parse_review((ROOT/'tests/sample_report_format5.md').read_text())
        d=json.loads((ROOT/'tests/sample_grounded_lesson.json').read_text())
        self.assertEqual(validate(p,d),([],[]))
        self.assertIn('Find a move that meets the goal',generate_html(p,d))
        legacy=parse_review((ROOT/'tests/sample_report_format4.md').read_text())
        self.assertEqual(len(build_facts(legacy)['actual_moves']),len(legacy['moves']))

    def test_praise_without_rating_renders_and_existing_legacy_lessons_work(self):
        from parse_review import parse_review
        p=parse_review((ROOT/'tests/sample_report_format5.md').read_text())
        d=json.loads((ROOT/'tests/sample_lesson_v5.json').read_text())
        m=p['moves'][0];d['good_moves']=[{'move_number':1,'move':m['move'],'explanation':'Supported constructive praise.'}]
        self.assertIn('Supported constructive praise.',generate_html(p,d))
        self.assertNotIn('just for fun',generate_html(p,d))
        self.assertEqual(validate(p,d)[0],[])

if __name__=='__main__':unittest.main()
