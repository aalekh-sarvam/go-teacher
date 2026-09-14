# Skill tests

`sample_report_format4.md` is a real go_teacher report (format 4). To check the parser after a change:

```
python3 scripts/parse_review.py tests/sample_report_format4.md /tmp/parsed.json
python3 scripts/validate_lesson.py /tmp/parsed.json tests/sample_lesson.json
python3 scripts/generate_lesson.py /tmp/parsed.json tests/sample_lesson.json /tmp/lesson.html
```

All three must succeed. `sample_lesson.json` is a minimal lesson that exercises every optional field.
