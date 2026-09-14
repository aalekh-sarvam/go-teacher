# Skill tests

`sample_report_format4.md` is a real go_teacher report (format 4). To check the parser after a change:

```
python3 scripts/parse_review.py tests/sample_report_format4.md /tmp/parsed.json
python3 scripts/validate_lesson.py /tmp/parsed.json tests/sample_lesson.json
python3 scripts/generate_lesson.py /tmp/parsed.json tests/sample_lesson.json /tmp/lesson.html
```

All three must succeed. `sample_lesson.json` is a minimal lesson that exercises every optional field.

## Format 5 / schema 2

`sample_report_format5.md` and `sample_lesson_v5.json` use a deterministic fake engine and
explicitly labeled synthetic prose to exercise the interface. They are regression fixtures,
not a real student lesson or a benchmark of puzzle difficulty.

Run `python3 -m unittest discover -s tests -p 'test_*.py'` from the skill directory.
The tests cover old reports, new evidence joins, setup/White/pass, capture and ko, forbidden
numeric transcription, external puzzle source/solution fields and script-context escaping.
