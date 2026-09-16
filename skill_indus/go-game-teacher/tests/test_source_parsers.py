"""Inline-fixture tests for the SGF and OGS puzzle-source parsers. No network."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from go_rules import Position
from parse_sgf_problem import parse_sgf_problems, sgf_to_gtp, gtp_to_sgf, parse_sgf
from parse_ogs_puzzle import convert, letters_to_gtp, rank_hint

# Black to play, 9x9: black B8/C8/A7/C7 surround white B7; its last liberty is B6.
SGF_TWO_VARIATIONS = """(;GM[1]FF[4]SZ[9]PL[B]C[Black to play: capture the white stone.]
AB[bb][cb][ac][cc]AW[bc]
(;B[bd]C[Correct - white is captured.])
(;B[cd];W[bd]C[Wrong: White escapes.])
)"""

# No markers, branch point below ply 1, compressed AB, two games in one file.
SGF_COLLECTION = """(;SZ[9]AB[aa:ba]AW[ab];B[cb];W[bb](;B[ca])(;B[ac];W[ca]))
(;SZ[9]PL[W]AB[ee]AW[ed];W[de]C[Right])"""

OGS_FIXTURE = {
    "id": 45, "name": "Fixture corner", "owner": {"username": "someone"},
    "collection": {"id": 7, "name": "Fixtures"}, "private": False, "rating": 4.2, "type": "puzzle",
    "puzzle": {
        "puzzle_rank": "20", "puzzle_type": "life_and_death", "width": 9, "height": 9,
        "initial_state": {"black": "bbcbaccc", "white": "bc"}, "initial_player": "black",
        "puzzle_description": "Capture the white stone.",
        "move_tree": {"x": -1, "y": -1, "branches": [
            {"x": 1, "y": 3, "correct_answer": True, "text": "Atari from below; nothing escapes."},
            {"x": 2, "y": 3, "wrong_answer": True, "text": "White escapes.",
             "branches": [{"x": 1, "y": 3, "branches": [
                 {"x": 0, "y": 3, "wrong_answer": True, "text": "Too slow."}]}]},
        ]},
    },
}


def replay(puzzle, moves):
    """Replay a line with go_rules; raises on any illegal move."""
    p = Position(puzzle['board_size'], black=puzzle['black'], white=puzzle['white'],
                 to_move=puzzle['player_to_move'])
    for mv in moves:
        p.play(mv)
    return p


class CoordinateTests(unittest.TestCase):
    def test_sgf_letters_map_to_gtp_without_i_and_rows_from_bottom(self):
        self.assertEqual(sgf_to_gtp('da', 19), 'D19')
        self.assertEqual(sgf_to_gtp('as', 19), 'A1')
        self.assertEqual(sgf_to_gtp('ia', 19), 'J19')  # column 8 skips I
        self.assertEqual(sgf_to_gtp('ai', 9), 'A1')
        self.assertEqual(sgf_to_gtp('tt', 19), 'pass')
        self.assertEqual(sgf_to_gtp('', 9), 'pass')
        for pt in ('aa', 'sa', 'ss', 'jj'):
            self.assertEqual(gtp_to_sgf(sgf_to_gtp(pt, 19), 19), pt)
        with self.assertRaises(ValueError):
            sgf_to_gtp('ja', 9)

    def test_ogs_letter_pairs_are_x_then_y_from_top(self):
        self.assertEqual(letters_to_gtp('daaa', 19, 19), ['D19', 'A19'])
        self.assertEqual(letters_to_gtp('bbcbaccc', 9, 9), ['B8', 'C8', 'A7', 'C7'])
        self.assertEqual(letters_to_gtp('', 9, 9), [])
        with self.assertRaises(ValueError):
            letters_to_gtp('abc', 9, 9)


class SgfProblemTests(unittest.TestCase):
    def test_setup_player_and_variation_classification(self):
        problems = parse_sgf_problems(SGF_TWO_VARIATIONS, 'fixture.sgf')
        self.assertEqual(len(problems), 1)
        p = problems[0]
        self.assertEqual(p['board_size'], 9)
        self.assertEqual(p['player_to_move'], 'B')
        self.assertEqual(p['black'], ['B8', 'C8', 'A7', 'C7'])
        self.assertEqual(p['white'], ['B7'])
        self.assertEqual(p['sgf_black'], ['bb', 'cb', 'ac', 'cc'])
        self.assertEqual(p['description'], 'Black to play: capture the white stone.')
        self.assertEqual(p['correct_moves'], ['B6'])
        self.assertEqual(p['correct_lines'], {'B6': ['B6']})
        self.assertEqual(p['wrong_moves'], [{'move': 'C6', 'refutation': ['B6'],
                                             'explanation': 'Wrong: White escapes.'}])
        self.assertEqual(p['warnings'], [])
        self.assertEqual(p['source']['example_locator'], 'fixture.sgf problem 1')
        self.assertEqual(p['source']['position'],
                         {'board_size': 9, 'black_stones': ['B8', 'C8', 'A7', 'C7'],
                          'white_stones': ['B7'], 'player_to_move': 'B'})

    def test_lines_replay_legally_and_correct_line_captures(self):
        p = parse_sgf_problems(SGF_TWO_VARIATIONS, 'fixture.sgf')[0]
        after = replay(p, p['correct_lines']['B6'])
        self.assertNotIn(after.xy('B7'), after.board)  # white stone captured
        w = p['wrong_moves'][0]
        after = replay(p, [w['move']] + w['refutation'])
        self.assertEqual(after.board[after.xy('B7')], 'W')  # white still there

    def test_no_markers_defaults_to_first_variation_at_the_fork(self):
        problems = parse_sgf_problems(SGF_COLLECTION, 'two.sgf')
        self.assertEqual(len(problems), 2)
        first, second = problems
        self.assertEqual(first['black'], ['A9', 'B9'])  # compressed AB[aa:ba]
        self.assertEqual(first['white'], ['A8'])
        self.assertEqual(first['player_to_move'], 'B')  # inferred from first move
        self.assertEqual(first['correct_lines'], {'C8': ['C8', 'B8', 'C9']})
        self.assertEqual(first['wrong_moves'], [])  # both leaves share the first move
        self.assertTrue(any('first variation assumed' in w for w in first['warnings']))
        replay(first, first['correct_lines']['C8'])
        self.assertEqual(second['player_to_move'], 'W')
        self.assertEqual(second['correct_moves'], ['D5'])
        self.assertEqual(second['source']['problem'], 2)

    def test_incorrect_is_not_read_as_correct(self):
        sgf = "(;SZ[9]AB[bb]AW[bc](;B[bd]C[This is incorrect.])(;B[cd]C[correct]))"
        p = parse_sgf_problems(sgf)[0]
        self.assertEqual(p['correct_moves'], ['C6'])
        self.assertEqual([w['move'] for w in p['wrong_moves']], ['B6'])

    def test_escaped_brackets_and_collections_parse(self):
        games = parse_sgf("(;C[a \\] b];B[aa])(;B[bb])")
        self.assertEqual(len(games), 2)
        self.assertEqual(games[0].get('C'), 'a ] b')
        with self.assertRaises(ValueError):
            parse_sgf("(;B[aa]")

    def test_cli_writes_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / 'p.sgf'; src.write_text(SGF_COLLECTION)
            out = Path(tmp) / 'p.json'
            subprocess.run([sys.executable, str(ROOT / 'scripts/parse_sgf_problem.py'), str(src),
                            '--index', '1', '-o', str(out)], check=True, capture_output=True)
            data = json.loads(out.read_text())
            self.assertEqual(data['player_to_move'], 'W')
            self.assertEqual(data['source']['problem'], 2)


class OgsPuzzleTests(unittest.TestCase):
    def test_conversion_fields(self):
        p = convert(OGS_FIXTURE)
        self.assertEqual(p['title'], 'Fixture corner')
        self.assertEqual(p['board_size'], 9)
        self.assertEqual(p['player_to_move'], 'B')
        self.assertEqual(p['black'], ['B8', 'C8', 'A7', 'C7'])
        self.assertEqual(p['white'], ['B7'])
        self.assertEqual(p['correct_moves'], ['B6'])
        self.assertEqual(p['correct_lines'], {'B6': ['B6']})
        self.assertEqual(p['wrong_moves'], [{'move': 'C6', 'refutation': ['B6', 'A6'],
                                             'explanation': 'Too slow.'}])
        self.assertEqual(p['source']['url'], 'https://online-go.com/api/v1/puzzles/45')
        self.assertEqual(p['source']['example_locator'], 'OGS puzzle 45 (Fixture corner)')
        self.assertEqual(p['source']['owner'], 'someone')
        self.assertEqual(p['source']['collection_id'], 7)
        self.assertEqual(p['source']['position']['black_stones'], ['B8', 'C8', 'A7', 'C7'])
        self.assertEqual(p['rank_hint']['puzzle_rank'], '20')
        self.assertIn('unconfirmed', p['rank_hint']['note'].lower())
        self.assertEqual(p['warnings'], [])

    def test_lines_replay_legally(self):
        p = convert(OGS_FIXTURE)
        after = replay(p, p['correct_lines']['B6'])
        self.assertNotIn(after.xy('B7'), after.board)
        w = p['wrong_moves'][0]
        replay(p, [w['move']] + w['refutation'])

    def test_white_to_play_pass_and_missing_flags(self):
        data = json.loads(json.dumps(OGS_FIXTURE))
        data['puzzle']['initial_player'] = 'white'
        data['puzzle']['move_tree'] = {"x": -1, "y": -1, "branches": [
            {"x": -1, "y": -1, "branches": [{"x": 0, "y": 8, "text": "unflagged"}]}]}
        p = convert(data)
        self.assertEqual(p['player_to_move'], 'W')
        self.assertEqual(p['correct_moves'], [])
        self.assertEqual(p['wrong_moves'][0]['move'], 'pass')
        self.assertEqual(p['wrong_moves'][0]['refutation'], ['A1'])
        self.assertTrue(any('no branch flagged' in w for w in p['warnings']))

    def test_rank_hint_marks_zero_unranked(self):
        self.assertEqual(rank_hint('0')['tentative_label'], 'unranked')
        self.assertNotIn('tentative_label', rank_hint(None))
        self.assertTrue(rank_hint('35')['tentative_label'].startswith('6d'))

    def test_cli_reads_local_file_without_network(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / 'ogs45.json'; src.write_text(json.dumps(OGS_FIXTURE))
            out = Path(tmp) / 'puzzle.json'
            run = subprocess.run([sys.executable, str(ROOT / 'scripts/parse_ogs_puzzle.py'), str(src),
                                  '-o', str(out)], check=True, capture_output=True, text=True)
            self.assertIn('credit', run.stderr)
            self.assertEqual(json.loads(out.read_text())['correct_moves'], ['B6'])


if __name__ == '__main__':
    unittest.main()
