"""Regression tests for report ingestion, evidence joins and legal lesson/puzzle playback."""
import copy
import json
import re
import sys
import unittest
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from parse_review import parse_review
from lesson_contract import hydrate, brief
from validate_lesson import validate, canonical
from go_rules import Position, sequence_position
from generate_lesson import generate_html

class SkillTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parsed=parse_review((ROOT/'tests/sample_report_format5.md').read_text())
        cls.lesson=json.loads((ROOT/'tests/sample_lesson_v5.json').read_text())

    def test_report_and_lesson_round_trip(self):
        errors,warnings=validate(self.parsed,self.lesson)
        self.assertEqual(errors,[])
        html=generate_html(self.parsed,self.lesson)
        self.assertIn('sampled human policy',html)
        self.assertIn('setPerspective',html)
        self.assertNotIn('btn-pref-',html)

    def test_old_format_still_parses(self):
        parsed=parse_review((ROOT/'tests/sample_report_format4.md').read_text())
        self.assertEqual(parsed['report_format'],4)
        self.assertTrue(parsed['moves'])

    def test_context_additions_survive_parser_and_brief_without_changing_lesson(self):
        updated=copy.deepcopy(self.parsed)
        updated['initial_position']={'turn':0,'score_black':-6.5,'winrate_black':0.4,'visits':150}
        updated['moves'][0].update(point_loss=1.25,score_black=-7.75,time_spent_seconds=0.0)
        updated['summary']['timing']={'status':'available','players':{'B':{'timed_moves':1}}}
        text='Report format: 5\n\n```go-teacher-evidence\n'+json.dumps(updated)+'\n```\n'
        parsed=parse_review(text)
        view=brief(parsed)
        self.assertEqual(view['moves'],updated['moves'])
        self.assertEqual(view['initial_position'],updated['initial_position'])
        self.assertEqual(view['summary'],updated['summary'])
        self.assertNotIn('move_details',view)
        self.assertEqual(parsed,updated)  # Brief must not mutate the authoritative evidence.
        self.assertEqual(hydrate(parsed,self.lesson),hydrate(self.parsed,self.lesson))
        self.assertEqual(validate(parsed,self.lesson)[0],[])
        before=generate_html(self.parsed,self.lesson); after=generate_html(parsed,self.lesson)
        for variable in ('lessonData','puzzleData','goodMoveData'):
            pattern=rf'const {variable} = (.*);'
            self.assertEqual(re.search(pattern,before)[1],re.search(pattern,after)[1])
        self.assertEqual(sequence_position(parsed,{'from_turn':len(parsed['moves']),'first_to_move':'B','moves':[]}).board,
                         sequence_position(self.parsed,{'from_turn':len(parsed['moves']),'first_to_move':'B','moves':[]}).board)

    def test_older_reports_keep_their_available_timeline_in_brief(self):
        for parsed in (self.parsed,parse_review((ROOT/'tests/sample_report_format4.md').read_text())):
            self.assertEqual(brief(parsed)['moves'],parsed['moves'])
        self.assertNotIn('initial_position',brief(self.parsed))

    def test_truncated_or_duplicate_blocks_rejected(self):
        text=(ROOT/'tests/sample_report_format5.md').read_text()
        with self.assertRaises(ValueError):parse_review(text[:-5])
        with self.assertRaises(ValueError):parse_review(text+'\n'+text)

    def test_no_model_transcription(self):
        lesson=copy.deepcopy(self.lesson);lesson['lessons'][0]['point_loss']=999
        errors,_=validate(self.parsed,lesson)
        self.assertTrue(any('prose only' in e for e in errors))

    def test_unknown_evidence_reference(self):
        lesson=copy.deepcopy(self.lesson);lesson['lessons'][0]['move_number']=999
        self.assertTrue(validate(self.parsed,lesson)[0])

    def test_setup_white_and_pass(self):
        parsed={'game_info':{'board_size':9,'setup_black':['C7','G3'],'initial_player':'W','rules':'japanese'},'moves':[{'number':1,'move':'pass','color':'W'},{'number':2,'move':'D4','color':'B'}]}
        seq={'from_turn':0,'first_to_move':'White','moves':['pass','D4','E4']}
        p=sequence_position(parsed,seq)
        self.assertEqual(p.board[p.xy('C7')],'B');self.assertEqual(p.board[p.xy('E4')],'W')
        seq['first_to_move']='Black'
        with self.assertRaises(ValueError):sequence_position(parsed,seq)

    def test_capture_frees_point_in_refutation(self):
        p=Position(9,black=['A2','B1','C2'],white=['B2','A3','C3','B4'])
        p.play('B3')
        with self.assertRaises(ValueError):p.play('B2')
        p.play('pass');p.play('H9');p.play('B2')
        self.assertNotIn(p.xy('B3'),p.board)

    def test_suicide_and_bounds(self):
        p=Position(9,black=['A2','B1','C2','B3'],to_move='W')
        for move in ['B2','I4','T3','C10','passx']:
            with self.assertRaises(ValueError):p.clone().play(move)
        p.play('pass');self.assertEqual(p.to_move,'B')

    def test_puzzle_needs_independent_source_and_solution(self):
        lesson=copy.deepcopy(self.lesson);lesson['puzzles'][0].pop('source');lesson['puzzles'][0]['correct_lines']={}
        errors,_=validate(self.parsed,lesson)
        self.assertTrue(any('source' in e for e in errors));self.assertTrue(any('correct_lines' in e for e in errors))

    def test_puzzle_capture_and_symmetry(self):
        puzzle=self.lesson['puzzles'][0]
        p=Position(9,black=puzzle['black_stones'],white=puzzle['white_stones'])
        self.assertEqual(len(p.group(p.xy('B2'))[1]),1)
        p.play('E4');p.play('D2')
        self.assertNotIn(p.xy('B2'),p.board);self.assertNotIn(p.xy('C2'),p.board)
        rotated={(8-y,x):'W' if c=='B' else 'B' for (x,y),c in p.board.items()}
        self.assertEqual(canonical(p.board,9,9),canonical(rotated,9,9))

    def test_script_delimiter_is_escaped(self):
        lesson=copy.deepcopy(self.lesson)
        lesson['lessons'][0]['panels']['played']='A </script><script>bad()</script> label'
        html=generate_html(self.parsed,lesson)
        self.assertEqual(html.count('</script>'),1)

if __name__=='__main__':unittest.main()
