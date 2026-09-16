"""Generator v6: colour-safe sequences, panel prose formatting, praise cards,
level labels from student_profile, glossary, and a parse check of the inline JS.

Fixture: the evidence JSON block of tests/sample_report_format5.md (evidence_version 1),
with the evidence_version 2 fields (student_profile, teaching.praise, *_with_colours)
added in memory so no fixture file changes are needed."""
import copy
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from parse_review import parse_review
from generate_lesson import generate_html, glossify, escape_html, format_paragraphs, GLOSSARY

JS_PATH = ROOT / 'scripts' / 'lesson_panels.js'


def load_report_json():
    """The fenced go-teacher-evidence block of the format-5 sample, parsed and derived."""
    return parse_review((ROOT / 'tests/sample_report_format5.md').read_text())


def run_js(source):
    """Evaluate JavaScript with the platform JavaScriptCore runner; None when unavailable."""
    if not shutil.which('osascript'):
        return None
    with tempfile.NamedTemporaryFile('w', suffix='.js', delete=False) as fh:
        fh.write(source)
        path = fh.name
    try:
        proc = subprocess.run(['osascript', '-l', 'JavaScript', path], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        return None
    finally:
        Path(path).unlink(missing_ok=True)
    if proc.returncode != 0:
        raise AssertionError('JavaScript failed: ' + proc.stderr.strip())
    return proc.stdout.strip()


def js_helpers():
    """The DOM-free helper functions from lesson_panels.js (prose + sequence formatting)."""
    src = JS_PATH.read_text()
    start = src.index('// ===== Prose formatting')
    end = src.index('// DOM version for the stepper')
    return src[start:end]


class GeneratorV6Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parsed = load_report_json()
        cls.lesson = json.loads((ROOT / 'tests/sample_lesson_v5.json').read_text())
        first, second = cls.parsed['moves'][0], cls.parsed['moves'][1]
        cls.profile = {
            'estimate': {'rank_value': -12, 'rank_label': '12k', 'low_label': '14k', 'high_label': '10k',
                         'wording': 'plays like a 12k in this game (range 14k–10k, 40 moves)'},
            'working_rank': {'rank_value': -11, 'rank_label': '11k', 'games': 3},
            'profiles_used': {'student': 'rank_10k', 'target': 'rank_3k'},
        }
        cls.praise = [{
            'move_number': first['number'], 'mv': first['move'], 'player': first['color'],
            'kind_label': 'capture', 'note': 'Captured two stones after the atari.',
            'gap': 3.25, 'stones_captured': 2, 'human_prob': 0.41,
            'rating': 'a move most 8k players find', 'rating_source': 'human-profile ladder: 8k policy 41%',
        }]
        cls.good_moves = [
            {'move_number': first['number'], 'move': first['move'], 'explanation': 'Good **timing** on the capture.'},
            # Author-supplied rating with no source and no praise entry: the badge must be dropped.
            {'move_number': second['number'], 'move': second['move'], 'explanation': 'Author praise only.',
             'rating': 'dan-level insight'},
        ]

    def v2_parsed(self):
        parsed = copy.deepcopy(self.parsed)
        parsed['evidence_version'] = 2
        parsed['student_profile'] = copy.deepcopy(self.profile)
        parsed['teaching']['praise'] = copy.deepcopy(self.praise)
        for cand in parsed['teaching']['candidates']:
            opp = 'W' if cand['player'] == 'B' else 'B'
            cand['refutation_with_colours'] = [f'{opp if k % 2 == 0 else cand["player"]} {mv}' for k, mv in enumerate(cand.get('refutation', []))]
            cand['better_line_with_colours'] = [f'{cand["player"] if k % 2 == 0 else opp} {mv}' for k, mv in enumerate(cand.get('better_line', []))]
            for seq in cand.get('evidence', {}).get('sequences', []):
                colour = 'W' if str(seq.get('first_to_move', 'B')).startswith('W') else 'B'
                coloured = []
                for mv in seq['moves']:
                    coloured.append(f'{colour} {mv}')
                    colour = 'W' if colour == 'B' else 'B'
                seq['moves_with_colours'] = coloured
        return parsed

    def render(self, parsed=None, lesson=None):
        return generate_html(parsed or self.v2_parsed(), lesson or copy.deepcopy(self.lesson))

    # 1. colour-safe sequences ------------------------------------------------
    def test_render_sequence_present_and_used(self):
        html = self.render()
        self.assertIn('function renderSequence(', html)
        self.assertIn('moves_with_colours', html)
        self.assertIn("fact('Sequence: '+renderSequence(current))", html)
        self.assertIn('id="sequence-line-0"', html)
        # coloured lines from the candidate travel into the lesson data
        self.assertIn('"refutation_with_colours": ["W ', html)
        self.assertIn('"better_line_with_colours": ["B ', html)
        # the old bare-coordinate option label is gone
        self.assertNotIn("s.moves.slice(0,3).join", html)

    def test_sequence_tokens_runtime(self):
        script = js_helpers() + r"""
        const GLOSSARY = {};
        JSON.stringify([
          renderSequence({moves:['D7','E3','pass'], first_to_move:'Black'}),
          renderSequence({moves:['F2','D8'], first_to_move:'W'}),
          renderSequence({moves:['F2','D8'], first_to_move:'B', moves_with_colours:['W F2','B D8']}),
          renderSequence(null)
        ]);"""
        out = run_js(script)
        if out is None:
            self.skipTest('osascript JavaScript runner unavailable')
        self.assertEqual(json.loads(out), ['B D7 → W E3 → B pass', 'W F2 → B D8', 'W F2 → B D8', ''])

    # 2. panel prose ------------------------------------------------------------
    def test_panel_bold_becomes_strong(self):
        lesson = copy.deepcopy(self.lesson)
        lesson['lessons'][0]['panels']['position'] = 'Watch the **cutting point** here.\n\nSecond <b>para</b>.'
        html = self.render(lesson=lesson)
        # the panel reaches the page as data; the formatter, not textContent, renders it
        self.assertIn("explain.insertAdjacentHTML('beforeend', formatProse(text))", html)
        self.assertNotIn('p.textContent=paragraph', html)
        script = js_helpers() + r"""
        const GLOSSARY = {};
        JSON.stringify([formatProse('Watch the **cutting point** here.\n\nSecond <b>para</b>.'),
                        boldify(escapeHtml('unpaired ** stays')),
                        boldify(escapeHtml('a **b** c **d'))]);"""
        out = run_js(script)
        if out is None:
            self.skipTest('osascript JavaScript runner unavailable')
        first, unpaired, odd = json.loads(out)
        self.assertEqual(first, '<p>Watch the <strong>cutting point</strong> here.</p><p>Second &lt;b&gt;para&lt;/b&gt;.</p>')
        self.assertEqual(unpaired, 'unpaired ** stays')
        self.assertEqual(odd, 'a <strong>b</strong> c **d')

    def test_static_bold_unpaired_is_literal(self):
        self.assertEqual(escape_html('a **b** c'), 'a <strong>b</strong> c')
        self.assertEqual(escape_html('a **b'), 'a **b')
        self.assertEqual(escape_html('<x> **y**'), '&lt;x&gt; <strong>y</strong>')

    # 3. praise cards -----------------------------------------------------------
    def test_praise_card_shows_kind_source_and_hides_unsourced_rating(self):
        lesson = copy.deepcopy(self.lesson)
        lesson['good_moves'] = copy.deepcopy(self.good_moves)
        html = self.render(lesson=lesson)
        card = html[html.index('class="good-moves"'):html.index('class="puzzle"')]
        first = self.parsed['moves'][0]
        self.assertIn('good-moves-grid', card)
        self.assertIn('<span class="badge badge-kind">capture</span>', card)
        self.assertIn('Captured two stones after the', card)  # note under the title
        self.assertIn('Captured 2 stones', card)
        self.assertIn('3.2 points better than the next searched move', card)
        self.assertIn(f'Move {first["number"]}: {first["color"]} {first["move"]}', card)  # colour-safe title
        self.assertIn('<span class="rating-badge">a move most 8k players find</span>', card)
        self.assertIn('Source: human-profile ladder: 8k policy 41%', card)
        self.assertIn('Good <strong>timing</strong>', card)
        # unsourced author rating is dropped, no source line for it
        self.assertNotIn('dan-level insight', card)
        self.assertEqual(card.count('rating-badge'), 1)
        self.assertEqual(card.count('Source:'), 1)

    def test_praise_rating_needs_source_even_from_report(self):
        parsed = self.v2_parsed()
        parsed['teaching']['praise'][0].pop('rating_source')
        lesson = copy.deepcopy(self.lesson)
        lesson['good_moves'] = copy.deepcopy(self.good_moves[:1])
        html = self.render(parsed=parsed, lesson=lesson)
        self.assertNotIn('rating-badge">a move most', html)
        self.assertIn('badge-kind">capture', html)

    # 4. level label --------------------------------------------------------------
    def test_level_label_from_student_profile(self):
        html = self.render()
        bar = html[html.index('class="perspective-bar"'):html.index('</section>', html.index('class="perspective-bar"'))]
        self.assertIn('Your level · 12k', bar)
        self.assertIn('Target level · 3k', bar)
        self.assertIn('plays like a 12k in this game', bar)  # wording as small text
        self.assertIn("Your level is this game's human-profile estimate", bar)  # source line
        self.assertIn('sampled with the 10k profile', bar)
        progress = html[html.index('class="progress"'):]
        self.assertIn('Working rank: <strong>11k</strong>', progress)
        self.assertIn('last 3 analysed games', progress)

    def test_level_label_falls_back_to_profiles(self):
        html = generate_html(copy.deepcopy(self.parsed), copy.deepcopy(self.lesson))
        self.assertIn('Your level · 10k', html)
        self.assertIn('Target level · 3k', html)
        self.assertIn('Levels are the human profiles used for this analysis', html)
        self.assertNotIn('Working rank:', html)
        parsed = copy.deepcopy(self.parsed)
        parsed['profiles'] = {}
        html = generate_html(parsed, copy.deepcopy(self.lesson))
        self.assertIn('No level estimate is available', html)
        self.assertIn('>Your level</button>', html)

    # 5. panels and dead code -----------------------------------------------------
    def test_panel_labels_and_no_old_buttons(self):
        html = self.render()
        for label in ('Position', 'Your move and what it allowed', 'A better plan', 'Other choices, graded',
                      'Read the local fight', 'What would likely happen next'):
            self.assertIn(f'<strong>{label}</strong><span>', html)
        self.assertNotIn('btn-alt-', html)
        self.assertNotIn('alt-controls', html)
        self.assertNotIn('Show played move', html)
        self.assertNotIn('Show better move', html)

    # 6. glossary -------------------------------------------------------------------
    def test_glossary_wraps_first_atari_only(self):
        lesson = copy.deepcopy(self.lesson)
        lesson['lessons'][0]['story'] = 'An atari here, then another atari there. Atari is not "Ataris".'
        lesson['lessons'][0]['principle'] = 'Count liberties before the atari.'
        lesson['overall_feedback'] = 'You played sente moves & kept sente; kyusho matters, so does kyūsho.'
        html = self.render(lesson=lesson)
        story = html[html.index('class="lesson-story"'):html.index('</div>', html.index('class="lesson-story"'))]
        self.assertEqual(story.count('<abbr title='), 1)
        self.assertIn('<abbr title="' + escape_html(GLOSSARY['atari']) + '">atari</abbr> here, then another atari', story)
        principle = html[html.index('Key principle:'):html.index('</div>', html.index('Key principle:'))]
        self.assertEqual(principle.count('<abbr'), 1)
        overview = html[html.index('class="overview-text"'):html.index('</section>', html.index('class="overview-text"'))]
        self.assertEqual(overview.count('>sente</abbr>'), 1)
        self.assertEqual(overview.count('>kyusho</abbr>'), 1)
        self.assertEqual(overview.count('kyūsho</abbr>'), 0)
        self.assertIn('&amp; kept sente', overview)
        # unit level: tags and attribute text are never touched, matching is whole-word
        self.assertEqual(glossify('<strong>ko</strong> and ko'), '<strong><abbr title="' + escape_html(GLOSSARY['ko']) + '">ko</abbr></strong> and ko')
        self.assertEqual(glossify('KOMI is not ko-related'), 'KOMI is not <abbr title="' + escape_html(GLOSSARY['ko']) + '">ko</abbr>-related')
        self.assertIn('const GLOSSARY = {', html)  # the panels apply the same glossary at runtime

    def test_glossary_runtime_matches_static(self):
        script = js_helpers() + '\nconst GLOSSARY = ' + json.dumps(GLOSSARY, ensure_ascii=False) + ';\n' + \
            r"""JSON.stringify(formatProse('An atari, then atari. **ko** fights.'));"""
        out = run_js(script)
        if out is None:
            self.skipTest('osascript JavaScript runner unavailable')
        rendered = json.loads(out)
        self.assertEqual(rendered.count('<abbr'), 2)
        self.assertIn('<abbr title="' + GLOSSARY['atari'].replace('"', '&quot;') + '">atari</abbr>, then atari.', rendered)
        self.assertIn('<strong><abbr title=', rendered)

    # 7. inline JS parses ------------------------------------------------------------
    def test_inline_js_parses(self):
        lesson = copy.deepcopy(self.lesson)
        lesson['good_moves'] = copy.deepcopy(self.good_moves)
        html = self.render(lesson=lesson)
        scripts = re.findall(r'<script>(.*?)</script>', html, re.S)
        self.assertEqual(len(scripts), 1)
        self.assertNotIn('</script', scripts[0])
        with tempfile.NamedTemporaryFile('w', suffix='.js', delete=False) as fh:
            fh.write(scripts[0])
            src_path = fh.name
        try:
            runner = (f"ObjC.import('Foundation');"
                      f"const src = ObjC.unwrap($.NSString.stringWithContentsOfFileEncodingError('{src_path}', $.NSUTF8StringEncoding, null));"
                      "new Function(src); 'parsed-ok'")
            out = run_js(runner)
        finally:
            Path(src_path).unlink(missing_ok=True)
        if out is None:
            self.skipTest('osascript JavaScript runner unavailable')
        self.assertEqual(out, 'parsed-ok')


if __name__ == '__main__':
    unittest.main()
