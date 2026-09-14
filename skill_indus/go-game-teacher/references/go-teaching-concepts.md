# Go Teaching Concepts

How to analyze KataGo review data, categorize mistakes by theme, explain them to beginners, and design practice puzzles for each theme.

## Table of contents

1. [Mistake themes](#mistake-themes)
2. [How to identify each theme from KataGo data](#identifying-themes)
3. [How to explain each theme to a beginner](#explaining-themes)
4. [Puzzle design per theme](#puzzle-design)
5. [Choosing which mistakes to teach](#choosing-mistakes)

---

## Mistake themes

Most beginner mistakes fall into one of these categories. When selecting 2-3 teaching moments, aim for distinct themes so each lesson teaches something new.

### 1. Direction of play

Choosing the wrong direction to develop — playing on the side where the opponent is strong instead of where you have influence, or attacking from the wrong side.

**KataGo signal**: The preferred move is far from the played move (different quadrant of the board), and the played move is close to opponent strength. Often the preferred move develops on the open side or approaches from the correct direction.

### 2. Shape and connection

Playing a move that creates bad shape (empty triangle, dumpling, heavy stones) when a better shape was available. Or failing to connect stones that could be cut.

**KataGo signal**: Preferred move is nearby (same area) but at a key shape point. The played move may be too close (redundant) or too far (disconnected). PV often shows the opponent exploiting the bad shape with a cutting point.

### 3. Reading and tactics

Not reading out a tactical sequence correctly — missing an atari, a capture, a ladder, or a tesuji. The move played loses material or gets captured.

**KataGo signal**: Preferred move responds to a local tactical threat. The played move ignores it or plays a different local move. The PV after the played move shows immediate material loss. Point loss is often large (5+ pts).

### 4. Timing of invasions

Invading too early (before establishing sufficient thickness) or too late (after the territory is settled). Also includes invading the wrong area.

**KataGo signal**: Preferred move is in a completely different area (establishing thickness, defending, or playing on the outside). The played move is deep in opponent territory. Winrate drops sharply. The PV shows the opponent sealing in the invading stone.

### 5. Endgame counting

Playing endgame moves in the wrong order — small points before big points, or missing a sente endgame move.

**KataGo signal**: Preferred move is elsewhere on the board with a larger point value. Both moves are in the endgame phase. Point loss is small (1-3 pts) but consistent.

### 6. Responding to threats (tenuki)

Ignoring a local threat when you should respond, or responding when you should play elsewhere (over-defending).

**KataGo signal**: The previous opponent move created a threat. The preferred move responds to it. The played move goes elsewhere (tenuki). The PV shows the threat materializing. Or the reverse: the opponent's move was not a real threat, and the preferred move develops elsewhere.

### 7. Overplay and over-concentration

Playing too many stones in one area (over-concentration) or trying too hard in a position that doesn't warrant aggression.

**KataGo signal**: The played move is near existing friendly stones that are already strong. The preferred move is on the opposite side or a new area. The position diagram shows a cluster of friendly stones where the played move is redundant.

---

## Identifying themes

To categorize a mistake:

1. **Look at the board diagram** (if available). Is the played move near strong opponent stones? Near your own strong stones? Deep in opponent territory? In the center?

2. **Compare played move to preferred move**. Are they in the same area (shape/tactics) or different areas (direction/timing)?

3. **Check the PV**. Does it show immediate material loss (tactics)? Gradual territorial loss (direction)? A sealed-in stone (invasion)?

4. **Check the phase**. Opening mistakes are usually direction or shape. Middlegame mistakes are often tactics or invasion timing. Endgame mistakes are counting.

5. **Check the winrate trajectory**. A sudden drop suggests a tactical miss. A gradual decline that accelerates suggests direction or invasion.

---

## Explaining themes

For each theme, here's how to explain it to a beginner:

### Direction of play
"Think about where you're strongest and where your opponent is strongest. You want to play near your opponent's weak stones (to attack them) and far from their strong stones (to avoid being squeezed). When you have a wall or thick position, play on the side it faces — your influence makes that territory yours for free."

### Shape and connection
"Good shape means your stones work together efficiently. A 'bamboo joint' (two stones with a one-space gap) is connected and hard to cut. An 'empty triangle' (three stones in an L with no stone at the joint) is wasteful and weak. Before playing, ask: does this move make my stones work better together, or does it pile them up?"

### Reading and tactics
"Before you play, read out what happens next. If your stone can be captured in two moves, don't play it unless you have a follow-up. Count the liberties — a stone with one liberty is in atari and will be captured. Practice reading 3-4 moves deep before committing to a local fight."

### Timing of invasions
"Don't invade until you have enough support nearby. If you invade deep into opponent territory with no nearby stones, they'll surround and capture you. First build thickness on the outside, then invade — or let your opponent come to you. A good rule: if your nearest stone is more than 5 points away, don't invade yet."

### Endgame counting
"In the endgame, count the value of each move. A move that secures 3 points is better than one that secures 2. Play the biggest moves first. Also watch for sente moves — moves that force your opponent to respond. You can play these for free and still come back to the big points."

### Responding to threats
"Before you play elsewhere, check if your opponent's last move threatens something. If a group has only one liberty, you must respond. If a stone can be cut off, you must connect. But don't over-defend — if there's no real threat, play where you can gain the most."

### Overplay
"Don't pile too many stones in one place. Three stones in a row is usually enough to be strong. If you add a fourth or fifth stone to an already strong position, you're wasting moves that could gain new territory. Spread your stones to cover more of the board."

---

## Puzzle design

For each theme, design 1-2 puzzles with fresh positions (not from the game) that test the same concept.

### General puzzle principles

- Keep it simple: 9x9 board, or show just a corner/side fragment
- One concept per puzzle: the puzzle should have one clear right answer
- Include 1-2 tempting wrong answers that illustrate the mistake
- The explanation should connect back to the lesson's principle
- Provide a hint that guides without giving away the answer

### Direction of play puzzles
- Set up a position where one side has a wall or strong group
- Offer 2-3 plausible-looking moves on different sides of the board
- The correct move develops on the correct side (away from opponent strength, toward open area)
- Wrong moves play into the opponent's strength

### Shape puzzles
- Set up a position with a shape decision: connect or not? Which point makes good shape?
- Include the empty triangle mistake as a wrong answer
- The correct move creates a good shape (bamboo joint, tiger's mouth, etc.)

### Tactics puzzles
- Set up a position where a stone or group can be captured or saved
- Include the wrong move that fails to read out the sequence
- The correct move shows the tactical solution (atari, capture, escape)

### Invasion timing puzzles
- Set up a position with established territory on one side and open area on another
- The wrong move invades the territory prematurely
- The correct move develops on the open side or builds thickness

### Endgame puzzles
- Set up a late-game position with several endgame moves available
- Include moves of different values
- The correct move is the largest point value

### How to specify puzzle positions

In the lesson JSON, each puzzle has `black_stones` and `white_stones` arrays with GTP coordinates, plus `correct_moves` (array of accepted answers). Example:

```json
{
  "title": "Which direction?",
  "concept": "direction_of_play",
  "concept_label": "Direction of Play",
  "board_size": 9,
  "black_stones": ["C3", "D3", "C4"],
  "white_stones": ["G6", "G7", "H6"],
  "opponent_last_move": "H6",
  "player_to_move": "B",
  "correct_moves": ["G3"],
  "hint": "Look at where White is strong and where the board is open.",
  "explanation": "White has a strong group in the upper right. Playing near them (like G3 or H3) develops the open lower-right side. Playing at E5 or F5 would be too close to White's strength and get squeezed."
}
```

---

## Choosing which mistakes to teach

When selecting 2-3 mistakes from a game:

1. **Sort by point loss** — the biggest mistakes are the most impactful
2. **Group by theme** — if moves 15 and 17 are both shape mistakes, pick the bigger one and find a different theme for the second lesson
3. **Prefer early/mid-game** — opening and middlegame mistakes are more instructive than endgame counting errors for beginners
4. **Look for instructive patterns** — a move that was "not searched by KataGo" is often a very unnatural move and worth teaching
5. **Consider the narrative** — pick mistakes that tell a story: "first you played too close, then you invaded too early, then you missed a capture". See `references/game-arc-commentary.md` for how to turn that story into the game-arc narrative and per-lesson cause→effect chains.
6. **Check for missed opportunities** — sometimes the most instructive moment is a move the player DIDN'T play that KataGo rated much higher

A good lesson set covers 2-3 distinct themes and gives the student actionable principles they can apply in their next game.

### Start from the program's shortlist

The report's **Teaching candidates** section already applies rules 1, 2 and 4: it ranks the student's losses, removes repeats from the same local fight, flags unsearched moves via the policy rank, and assigns a **theme** with the rules below (format 3). Take the theme as given unless the refutation or chain clearly shows something else, and say why when you override. The same rules, for reference:

| Hint | Likely theme |
|---|---|
| better move is "same area", stone captured later, groups in atari | Reading and tactics |
| better move is "same area", no capture | Shape and connection |
| better move is "elsewhere", KataGo's move answers the opponent's last move | Responding to threats (tenuki when threatened) |
| better move is "elsewhere", played move answers the opponent's last move | Over-defending / priority |
| better move is "elsewhere", opening phase | Direction of play |
| better move is "elsewhere", endgame, loss 1–3 | Endgame counting |
| played move deep in the region where points were lost | Timing of invasions / overplay |

### Decided games

When `decided_before` is true (Black winrate already under 5% or over 95%), the winrate swing tells you nothing: a 99% → 1% drop may be a 5-point mistake in a game where 5 points decide the result. Beginner games on 9x9 flip every move. Judge by point loss, prefer moments from the undecided part of the game if any exist, and never write "this move threw away the game" unless the winrate was genuinely live before it.

### Using the human-policy numbers

When a human profile was chosen, each move has "human policy": how often a player of that rank plays exactly this move, and the most common move at that rank.

- High human policy (say ≥ 15%) with a large loss: a **typical mistake for the level**. Teach it as a habit to unlearn, and contrast with what stronger players do (KataGo's move, or the most common human move if it is better).
- Very low human policy (≤ 1%) with a large loss: an **unusual slip**. Often a misclick or a misread; teach the reading, not the habit.
- The most common human move is often a good "human-sized" recommendation when KataGo's first choice is hard to explain. If it also has a low loss in the candidate table, prefer it as the teaching target and mention KataGo's move as the ideal.

### Explaining with the refutation

The most persuasive "why" is the punishment: the report's `refutation` is the opponent's strongest sequence after the played move, from a real search of that position. Walk it move by move ("1 takes the last outside liberty, 3 captures") and tie the result to the `refutation_score`. Contrast it with the `better_line`: the same fight after the right move. When the candidate has a `status_changes` entry, name the group that died or lived; that is the concrete consequence the student can see.

### Reading the chain

`chain_before` and `chain_after` list the moves in the same area with their losses. The real decision is often the first move in the chain with a large loss, not the shortlisted move; if the chain shows the student compounding one mistake with three more in the same area (each losing 8+ points), the lesson is "stop and reassess after a loss", not the individual moves. A `captured at move N` note on a chain move tells you where the story ends.

### Grading alternatives

The candidate table gives `Loss vs best` for each move KataGo searched. Use these thresholds for the `quality` field: best < 0.5, good < 1.5, inaccuracy < 3, mistake < 6, big mistake < 12, blunder ≥ 12. For each alternative you list, explain *what goes wrong* (the opponent's reply from the PV, the group left weak, the point left open), not only the number. A move with 1 visit has an unreliable number; say "KataGo barely looked at this" rather than quoting a precise loss.

### Puzzle variations that are not obvious

Puzzles must not reuse the game position. Build them from classic examples of the same idea found during research, then transform them so the student cannot answer by matching the lesson board:

1. Rotate or mirror the position, or move the fight to another side or corner.
2. Swap colours (the student may solve as White).
3. Add one or two stones that change the reading order without changing the lesson (an extra liberty, a stone that makes a ladder work or fail).
4. Change the surrounding context so the *principle* still decides the move (e.g. the same cut, but now the cutting stones have an escape route, so the correct move is to connect instead of capture).

Check coherence before writing the JSON: stones on distinct points inside the board, the opponent's last move present as a stone, the correct move on an empty point, every claimed atari really one liberty. For each puzzle, write 2–3 tempting wrong moves with their quality and why they fail; the student sees this feedback when they click one.

---

## Common Japanese Go terms

When writing the `concepts_learned` section, use these standard Japanese terms with their kanji and romaji:

| English | Japanese (Kanji + Romaji) | Meaning |
|---------|--------------------------|---------|
| Direction of play | 方向 (Hōkō) | Which side of the board to develop |
| Sente | 先手 | A move that forces the opponent to respond (initiative) |
| Gote | 後手 | A move that loses the initiative |
| Liberty | 呼吸点 (Kokyūten) | An empty point adjacent to a stone or group |
| Atari | 当たり | A stone or group with one liberty left (about to be captured) |
| Shape | 形 (Katachi) | The arrangement of stones and its efficiency |
| Thickness | 厚み (Atsumi) | Strong, solid influence that can't be easily disrupted |
| Influence | 勢力 (Seiryoku) | The reach of strong stones toward open areas |
| Territory | 地 (Ji) | Secured empty points that count toward the score |
| Invasion | 侵入 (Shinnyū) | Entering the opponent's area of influence |
| Tenuki | 手抜き | Playing elsewhere instead of responding locally |
| Tesuji | 手筋 | A skillful tactical move |
| Hane | 跳ね | A diagonal contact move turning around an opponent's stone |
| Bamboo joint | 杣 (Sugi) | Two stones one space apart that are virtually connected |
| Tiger's mouth | 虎口 (Toraguchi) | A shape with a single empty point that invites the opponent to fill it |
| Empty triangle | 愚形 (Gukei) | A wasteful shape of three stones in an L without the joint |
| Fuseki | 布石 | Opening theory; the first ~30-50 moves |
| Joseki | 定石 | A standard sequence of moves in a corner |
| Komi | コミ | Compensation points given to White for going second |
| Tsumego | 詰碁 | Life-and-death puzzles |
| Moyo | モヨ | A framework of influence that can be converted to territory |

---

## Go anecdotes and proverbs

When writing anecdotes for the `concepts_learned` section, draw from these categories:

### Famous players
- **Go Seigen (呉清源, 1914-2014)**: Revolutionized Go in the 1930s with "new fuseki" theory, emphasizing rapid development and playing on the open side rather than fixed corner patterns. His games changed how the entire Go world thinks about the opening.
- **Honinbo Shusaku (本因坊秀策, 1829-1862)**: Created the most famous joseki in Go — the Shusaku opening (3-3 point invasion of a 4-4 stone's corner). His games were considered so perfect that for a century, "playing like Shusaku" was the highest compliment.
- **Lee Sedol (이세돌)**: Lost 4-1 to AlphaGo in 2016, but his Game 4 victory — the "hand of God" move 78 — showed that human creativity can still find moves AI doesn't expect. He retired in 2019, saying AI had made the game less magical.
- **AlphaGo vs Ke Jie (2017)**: The world's #1 human player lost all three games, afterwards saying: "After humanity spent thousands of years improving our understanding of Go, AlphaGo shows us we've barely scratched the surface."

### Proverbs worth quoting
- "Play on the open side." — the most fundamental direction-of-play advice
- "A group with two eyes lives forever." — the core principle of life and death
- "Don't play go with your opponent's stones." — focus on your own groups
- "The enemy's key point is your key point." — often the vital point of a shape is the same for both sides
- "Sente gains nothing." — if you play a forcing move that doesn't actually accomplish anything, you've wasted the initiative
- "Urgent moves before big moves." — respond to threats before taking large territory

### Historical moments
- The "Blood Vomit Game" (1835): In a grudge match, Honinbo Jowa was reportedly poisoned but still won, coughing blood onto the board.
- The atomic bomb game (1945): The Hiroshima bomb interrupted a title match; the players finished the game in the ruins. The board is preserved in a museum.
- The "Ear Reddening Game" (1846): Shusaku's mentor noticed his ears turning red — a sign he'd found a brilliant move under pressure. He came back from a losing position to win.