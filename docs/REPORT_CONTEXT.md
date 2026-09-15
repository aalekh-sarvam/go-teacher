# Compact game context without losing teaching evidence

Implemented September 15, 2026. Rollback checkpoint: `3cc81fc`, pushed to `origin/main`
before implementation. This implements the agreed report/skill changes and prepares comparison
inputs; it does not run the user's teaching agent or establish improved lesson quality.

## What stays

All existing format-5 fields and values survive. In particular, teaching candidates retain their
commentary hints, position stones, policy probabilities, better lines, refutations, local chains,
themes, phase labels and deeper evidence. Praise, existing phase/status facts, openings, history,
missed opportunities, endgame values, human profiles and provenance are unchanged.

The detailed report is unchanged. Its selected full entries still cover teaching candidates,
praised moves, up to six opponent mistakes losing at least two points, checkpoints and the last
move. Full generated commentary exists for teaching candidates; it was never generated for
every move. Ownership status events are selected predictions/detections, not complete life/death
assessments or a complete capture ledger.

## What is added

- Each existing `moves[]` row gains `point_loss` for the mover and numeric `score_black` after
  the move. Initial evaluation is stored once in `initial_position`. Known thinking times add
  `time_spent_seconds`; unknown times do not become zero.
- `summary.accuracy.B/W` retains its old values and adds total/median loss, top-1/top-3 counts,
  searched-rank coverage and category counts. Aggregates clamp negative losses to zero; the
  timeline retains signed losses, which can reflect search disagreement. Median uses the two
  middle values for an even number of moves. Total loss is not the final score margin.
- `summary.timing` gives recorded/missing counts for each player, and, where available, mean
  and median thinking time and loss above/below that player's own median. Ties are in the lower
  bucket; an empty bucket has null mean loss. The bot's speed does not set the student's split.
- `context_moves` identifies opponent mistakes, checkpoints and the last move. Their evaluations
  and retained searched alternatives join the existing teaching/praise entries in `move_details`.
  Checkpoints are every 25 moves, or 50 for games exceeding 200 moves. Shared entries appear once.
  Ordinary move rows do not carry alternatives or repeated board diagrams.

No engine queries, model changes or new inference of group status are introduced. Existing saved
analysis can be rendered into the new format without new KataGo work. Existing Markdown files
on disk are not rewritten automatically; new analyses produce the new output. The supplied
comparison report was regenerated separately, leaving the user's saved files untouched.

Format 5 / evidence version 1 remains compatible: these are optional object fields and additional
move-detail entries, not renamed fields or a new encoding. No claim is made that raw time series
are inherently easier for GLM 5.3. Explicit coordinates, colours and score conventions remain.

## Skill changes

The Markdown is still the only game input. The parser retains the new optional fields and the
brief reading view now keeps the full compact move timeline, initial evaluation and summaries.
Exact selected alternatives/sequences are read from `parsed.json`, which also remains the
authoritative input for validation and HTML generation.

The skill keeps its existing lesson, arc, story, praise, progress, human perspectives, resources
and practice sections. It uses the new context to select a few broad observations, with most
attention on mistakes, replies and improvements. It distinguishes phase-level observations from
local tactical explanations and asks the agent to check intermediate scores before making a
claim about sustaining/surrendering a lead. A score dip alone cannot establish a missed capture,
and clock data cannot establish what the student thought or why they moved quickly.

Research remains uncapped. Puzzles still come from researched external examples with meaningful
variations and verified defences; actual game positions are not used as practice puzzles. No
new compulsory quiz is generated from a summary statistic. Older reports remain supported.

## Local checks and size cost

`scripts/check_report_context.py before.md after.md analysis.json` recursively verifies that
every old field/value survives and that teaching/arc/probe payloads are exactly unchanged. It
checks the new timeline, statistics, timing and supporting entries against the saved analysis,
and verifies the brief retains the context. Rendering both revisions from the same saved
analysis also confirmed identical detailed-report text.

| Saved analysis | Moves | Before bytes | After bytes | Added bytes | Growth |
|---|---:|---:|---:|---:|---:|
| September 14, 19:10 export | 94 | 47,397 | 60,223 | 12,826 | 27.1% |
| September 15, 00:20 export, no deeper probes | 68 | 15,423 | 25,793 | 10,370 | 67.2% |
| September 15, 01:14 export, with deeper probes | 68 | 52,045 | 61,653 | 9,608 | 18.5% |

These are three analysis snapshots of two games, not three independent games. The 68-move
report with deeper probes is the supplied comparison input and includes the user's Move 5
example. Its timeline adds 2,726 bytes and its supporting entries add 4,917 bytes; summaries,
field explanations and guidance account for the rest. All seven quoted Move 5 hints, both
stone lists, both lines and local chains were checked and are unchanged. Its baseline report
also matches the original saved Markdown byte-for-byte.

The additions are not free: the smaller export without deeper probes grows proportionally more.
Supporting positions account for much of the cost and were included at the user's request.
No alternatives were added to every move and no old evidence was cut to offset growth. These
are measured UTF-8 bytes, **not GLM 5.3 tokens**. Exact model input/token use must come from the
user's agent logs, including any reading of parsed/brief data, rather than just the upload size.

Additional checks:

- Rust: 16 tests passed, including signed loss, Black-view scores on White moves, handicap,
  passes, per-player timing medians, missing/zero clocks, empty buckets, empty records and
  checkpoint selection/deduplication. The pre-existing opt-in Metal test remained skipped;
  report changes need no new model inference.
- Python skill: 13 tests passed, including legacy reports, preservation of context in brief,
  unchanged hydrated lesson/puzzle data, legal replay and existing schema/source checks.
- Both fake-engine analysis snapshots also passed the source comparison: a timed game and
  a White-student handicap game. They are synthetic integration checks, not teaching examples.
- Skill validation, lesson JavaScript syntax, release build, app signing and ZIP contents checked.
- Two older saved JSON snapshots lack a `phase` field required by the existing deserializer and
  were not included; neither baseline nor updated exporter could load them. No historical files
  were changed or invented fields added to make them pass.

## What to run through the teaching system

Artifacts produced locally (not game-data uploads to GitHub):

- `dist/report-context-comparison.zip`: two reports, the old/new skill ZIPs, a shared prompt
  and instructions. Unzip it first.
- `skill_indus/go-game-teacher-v5-context.zip`: the updated skill to keep installed.
- `dist/GoTeacher-0.1.0.dmg`: the rebuilt app; replace the installed app through the DMG for
  future analyses. This build has not been copied into `/Applications` automatically.

Use the same model and settings, in two fresh conversations/workspaces:

1. Install `skill-before.zip`, upload **only `before.md`**, use `PROMPT.txt`.
2. Install `skill-after.zip`, upload **only `after.md`**, use the same prompt.
3. Share the two generated HTML lessons with Codex and say which explanations/puzzles were
   useful or confusing. Optional lesson JSON and agent logs help trace a missing fact, but are
   not required to start the comparison. Keep the updated skill installed afterward.

There is no need to rerun KataGo for these inputs, upload the detailed report or supply the
app's analysis JSON. If only trying the updated experience, run step 2 first. The archive itself
is not a game input and should not be uploaded wholesale as one lesson.

## How to improve it from the results

| Observed problem | Next adjustment |
|---|---|
| Missing or weaker tactical explanation | Check that the agent read the complete chosen entry from parsed.json; correct that reading step before adding more data. |
| Repetitive or oversized overview | Tighten prose guidance and distinguish overview/phase/story roles, preserving source evidence. |
| Unsupported capture, habit or lead claim | Inspect its move range and supporting chain/alternatives; correct the inference rather than deriving tactics from score alone. |
| Obvious or unsound practice puzzle | Revise the external example's meaningful transformation and verify the strongest defence on the new board. |
| High agent input cost | Inspect actual token/tool-reading logs for duplicated reads; measure proposed changes before trimming source evidence. |

This comparison can guide a narrow improvement, not prove general teaching quality. The agent's
research and generation may vary between runs. Local tests establish data preservation and
compatibility; the user's agent output and learning experience establish usefulness.
