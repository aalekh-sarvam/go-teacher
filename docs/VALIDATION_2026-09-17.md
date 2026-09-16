# Validation note, 2026-09-17: praise, level tailoring, parallel skill, deep coverage, UI layout

Three sources of requirements were implemented and checked against the eval set in
`evals/20260917_evals/` (9x9, student Black vs a 20k KaTrain bot, 66 moves, B+74.5):

1. The user's four observations (`docs/PLAN_2026-09-17_level_praise_parallel.md`, list A).
2. The 14 independent suggestions (list B of the same plan).
3. `docs/TEACHING_SEARCH_COVERAGE_HANDOFF.md`, sections 1–15 (deep-coverage guarantee, report
   contract, progress, skill, tests, UI layout).

The skill is executed by GLM 5.3; every skill change favours small inputs, fixed schemas and
validators that fail loudly.

## Commands run and results

```
cargo test                                  → 19 passed, 0 failed, 1 ignored (opt-in Metal smoke)
python3 -m unittest discover -s skill_indus/go-game-teacher/tests → 82 tests OK
unzip go-game-teacher-6.zip → independent dir → 82 tests OK
```

Real-engine run (KataGo 1.18.2 Metal, user's transformer network and config; upload profile
`rank_20k`, target `rank_15k`, two-pass 150 → 600 visits):

```
./target/release/go_teacher --out-dir /tmp/gt_verify3 analyze "evals/20260917_evals/KaTrain_Human (Normal Game) vs AI (Human-like) 2026-09-16 20 08 45.sgf" --two-pass --visits 150 --deep-visits 600 --human-profile rank_20k --human-profile-target rank_15k
```

Elapsed 512 s including 46 teaching-investigation queries and the 11-profile rank ladder.
Main report 117 KB (the evidence block carries the extra fields), detailed report unchanged in shape.

## Acceptance results on the eval game

| Check | Result |
|---|---|
| Student captures recorded | `stones_captured` 6 / 5 / 8 on moves 33 / 55 / 59 (were invisible before) |
| Praise fires | 5 praised: 31 only good move (45.5 pts gap), 33 capture ×6, 55 capture ×5, 59 capture ×8, 61 non-obvious best move |
| Ratings with a source | "a move most 20k players find first" (31, 33), "a move most 1d players find first" (59), "not the first choice of any tested human profile" (55, 61); `rating_source` names the human-profile ladder |
| Level estimate | "plays like a 18k in this game (range 21k–15k, 32 moves)"; opening 15k, middlegame 18k; endgame suppressed (too few moves) |
| Working rank | 18k after one game; drives the profiles when no upload/SGF rank exists |
| Decided-game framing | "decided from position 18 onward (final estimate B+75.5)"; `game_framing` in the evidence |
| Undo branches | `also_considered`: move 29 → D3; move 31 → D1, H1 (matches the SGF's undo variations) |
| Deep coverage | 7 of 7 candidates `complete` at 600 requested visits (actual 603); 0 catch-up rounds needed on this game; churn test proves the loop fires when the shortlist changes |
| Featured by importance | moments 29, 37, 41, 47 (losses 59.0, 28.3, 21.7, 24.0) instead of the earliest four |
| Coverage wording | table columns "Deep evaluation" and "Teaching investigations"; legacy `unavailable.deeper_search` replaced by `unavailable.teaching_investigations` |
| Turning points | move 2 (150-visit noise on an empty board) no longer listed |
| Themes | move 37 → priority, move 41 → shape (B3 hane); the "no move stood out" sentence is gone |
| Skill parser | reads evidence v2: profile wording, 5 praise entries with kinds and ratings, coverage summary 7×complete, coloured refutation tokens, also-considered |
| Slices | moment slice 37 KB, praise 12 KB, overview 34 KB, versus 212 KB parsed / 165 KB brief |

## What the coverage handoff asked for, item by item

| Handoff section | Status |
|---|---|
| §4 shared coverage model | `SearchRecord` on every `TurnEval` (purpose initial/deep/verification, requested visits, completed); `SearchCoverage`, `CandidateCoverage`, `PositionCoverage`; `position_deep()` is the single predicate used by the loop, the invariant, the reports, the live badge and the evidence |
| §5 catch-up loop + final invariant | `analyze_game`: initial broad deep pass retained; loop until every final candidate's two positions qualify; no-progress → error; final invariant errors with move numbers |
| §6 merge/resume/failure | `prefer_stronger()` used in the analysis and in the server's live/partial merge; a quick result never downgrades a deep one; cancel/failure paths unchanged |
| §7 featured by importance | `select_featured_candidates()` ranks by final loss, ties by move number, presents chronologically |
| §8 report/JSON contract | `search_coverage`, per-candidate `evaluation_coverage` and `investigation_coverage`, `unavailable.teaching_investigations`; two table columns; explanation sentence once; detailed `◆` from the same predicate |
| §9 progress/UI | phase text "verifying newly selected teaching moves: N of M candidates verified, K positions to search (round R)"; `◆ deep` badge from provenance; reopened JSON agrees with live |
| §10 skill | parser/brief/facts/validator understand both coverage fields, legacy key read narrowly; docs updated |
| §11 tests | churn test with `GT_FAKE_CHURN=1` (fake engine shifts its curve at deep budgets); skill `test_coverage.py` (11 tests) |
| §15 UI layout | panes rebalanced with a measured stacking breakpoint, candidate table fits its pane (bars below numbers, right-aligned numerics, scroll fallback with a label), grouped evaluation summary shared by mainline and variation modes, path disclosures in Analyses, nowrap sizes in Report files, settings collapse once ready |

Deviation flagged: the handoff prefers not to bump the evidence version; version 2 was already
introduced for the praise/profile fields, so the coverage fields live under version 2. The skill
accepts 1 and 2. The report format stays 5.

## The 14 suggestions

| # | Suggestion | Where |
|---|---|---|
| 1 | Colours in every sequence | `*_with_colours` in evidence; `renderSequence()` in the lesson page; SKILL.md token rule |
| 2 | SGF undo branches as evidence | `Move.also_considered`, `moves[].also_considered`, candidate hint |
| 3 | Decided-game framing | `game_framing` + report line |
| 4 | Deduplicate progress history | `same_game()`, newest analysis kept, `file_stem` filled, working rank |
| 5 | Validator parity with the generator | `validate_lesson.py` checks every key the generator dereferences |
| 6 | Stale UI docs, dead code, forbidden fields | html-build-guide rewritten; `alt_buttons` removed; SKILL.md no longer asks for `alternatives`/`played_quality`/`concept` |
| 7 | Bold in panels | panel prose through the bold/paragraph formatter |
| 8 | Non-featured candidates | coverage fields say "complete / not selected"; skill treats them as teachable with fewer panels |
| 9 | Puzzle/lesson theme consistency | validator warning on theme mismatch |
| 10 | Per-moment facts output | `parse_review.py --moves` |
| 11 | Jargon glossary | `<abbr>` on first use of 13 terms |
| 12 | Progress card from the same source | history deduplicated at the source; generator hydrates from parsed praise/profile |
| 13 | Suppress under-sampled phase estimates | estimate is `None` below 8 samples |
| 14 | Curated anecdotes only | concepts reference is the only allowed anecdote source |

## Remaining manual checks

- Open the rebuilt app at about 960 px width and confirm the Policy column sits inside the card,
  the evaluation groups wrap, and the settings card collapses once the engine is ready.
- Run the new skill (`skill_indus/go-game-teacher-6.zip`) on `/tmp/gt_verify3/*.md` (or a fresh
  analysis) in the teaching agent and check that the lesson praises moves 31/33/59 with the ladder
  ratings, uses "plays like an 18k", narrates sequences with colours, and does not call any candidate
  unverified.
- Notarization needs a Developer ID; the DMG is ad-hoc signed.
