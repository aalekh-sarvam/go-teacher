#!/usr/bin/env python3
"""A stand-in for `katago analysis` used by the integration tests: answers every query with
plausible, deterministic numbers so the pipeline can be exercised without a GPU or a model.

Supports: `version` (prints a Metal-like banner), `analysis` (JSON lines protocol), `benchmark`.
"""
import json, sys, math, os
# GT_FAKE_CHURN=1: at budgets >= 500 visits the evaluation curve shifts by a few turns, so the
# quick-pass shortlist and the deep-pass shortlist differ (exercises candidate verification).
CHURN = os.environ.get("GT_FAKE_CHURN") == "1"
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "skill_indus/go-game-teacher/scripts"))
from go_rules import Position

if len(sys.argv) > 1 and sys.argv[1] == "version":
    print("KataGo v9.9.9-fake\nUsing Metal backend"); sys.exit(0)
if len(sys.argv) > 1 and sys.argv[1] == "benchmark":
    print("numSearchThreads = 4: 10 / 10 positions, visits/s = 1234.5"); sys.exit(0)

COLS = "ABCDEFGHJKLMNOPQRST"

def cand_moves(sx, sy, turn):
    pts = []
    for k in range(6):
        x = (3 + 5 * k + turn) % sx
        y = (2 + 3 * k + 2 * turn) % sy
        pts.append(f"{COLS[x]}{sy - y}")
    return pts

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    q = json.loads(line)
    if q.get("action") == "terminate":
        continue
    if (q.get("overrideSettings") or {}).get("humanSLProfile") == "test_silent":
        continue
    sx, sy = q.get("boardXSize", 19), q.get("boardYSize", 19)
    moves = q.get("moves", [])
    n = sx * sy
    # allowMoves: restrict candidates to the listed moves for the side to move (local searches)
    allow = {d.get("player"): set(d.get("moves", [])) for d in (q.get("allowMoves") or [])}
    for turn in q.get("analyzeTurns", [len(moves)]):
        # side to move follows the actual move list (passes included); initialPlayer for turn 0
        if turn == 0:
            to_move = q.get("initialPlayer", "B")
        else:
            prev = moves[turn - 1][0] if turn - 1 < len(moves) else ("B" if turn % 2 == 1 else "W")
            to_move = "W" if prev == "B" else "B"
        setup=q.get("initialStones", [])
        board=Position(sx,sy,[m for c,m in setup if c=="B"],[m for c,m in setup if c=="W"],q.get("initialPlayer","B"),q.get("rules","japanese"))
        try:
            for color,mv in moves[:turn]:
                board.to_move=color;board.play(mv)
        except ValueError as e:
            print(json.dumps({"id":q["id"],"error":str(e)}),flush=True);continue
        board.to_move=to_move
        legal=[]
        for k in range(n+1):
            mv="pass" if k==n else f"{COLS[k%sx]}{sy-k//sx}"
            try: board.clone().play(mv);legal.append(mv)
            except ValueError: pass
        occupied={f"{COLS[x]}{sy-y}" for x,y in board.board}
        budget = q.get("maxVisits", 100)
        phase_shift = 1.7 if (CHURN and budget >= 500) else 0.0
        wr = 0.5 + 0.3 * math.sin(turn / 3.0 + phase_shift)
        # a pass by the side that just moved costs it points (so pass-value probes are meaningful)
        if turn >= 1 and turn - 1 < len(moves) and moves[turn - 1][1] == "pass":
            wr += -0.08 if moves[turn - 1][0] == "B" else 0.08
        lead = (wr - 0.5) * 30
        cands = []
        pool = [m for m in cand_moves(sx, sy, turn) if m in legal] or legal[:6]
        if to_move in allow:
            pool = [m for m in legal if m in allow[to_move]][:6] or ["pass"]
        for i, mv in enumerate(pool):
            c = {"move": mv, "visits": max(1, 100 - 30 * i), "winrate": max(0.01, min(0.99, wr - (1 if to_move=="B" else -1) * 0.02 * i)),
                 "scoreLead": lead - (1 if to_move=="B" else -1) * 1.5 * i, "scoreMean": lead - (1 if to_move=="B" else -1) * 1.5 * i, "prior": 0.4 / (i + 1), "order": i,
                 "pv": [mv], "utility": (wr - 0.5) * 2 - 0.05 * i}
            if q.get("includeMovesOwnership"):
                c["ownership"] = [round(math.tanh((k % sx - sx / 2 + i) / 4.0), 3) for k in range(n)]
            cands.append(c)
        if turn < len(moves) and to_move not in allow:
            played = moves[turn][1]
            if played not in [c["move"] for c in cands] and played != "pass":
                cands.append({"move": played, "visits": 2, "winrate": max(0.01, wr - 0.2), "scoreLead": lead - 6, "scoreMean": lead - 6,
                              "prior": 0.01, "order": len(cands), "pv": [played]})
        ownership = [round(math.tanh((i % sx - sx / 2) / 4.0), 3) for i in range(n)]
        profile=(q.get("overrideSettings") or {}).get("humanSLProfile", "")
        weights=[-1.0]*(n+1)
        for k in range(n+1):
            mv="pass" if k==n else f"{COLS[k%sx]}{sy-k//sx}"
            if mv in legal: weights[k]=0.0001 if mv=="pass" else 1.0+((k+sum(map(ord,profile)))%7)
        mass=sum(v for v in weights if v>0)
        policy=[v/mass if v>=0 else -1.0 for v in weights]
        human=policy[:]
        resp = {"id": q["id"], "turnNumber": turn, "isDuringSearch": False,
                "rootInfo": {"currentPlayer": to_move, "winrate": wr, "scoreLead": lead, "scoreSelfplay": lead, "scoreStdev": 8.0, "visits": sum(c["visits"] for c in cands), "utility": 0.0},
                "moveInfos": cands, "ownership": ownership, "policy": policy}
        if (q.get("overrideSettings") or {}).get("humanSLProfile"):
            resp["humanPolicy"] = human
        sys.stdout.write(json.dumps(resp) + "\n"); sys.stdout.flush()
