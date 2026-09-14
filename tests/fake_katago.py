#!/usr/bin/env python3
"""A stand-in for `katago analysis` used by the integration tests: answers every query with
plausible, deterministic numbers so the pipeline can be exercised without a GPU or a model.

Supports: `version` (prints a Metal-like banner), `analysis` (JSON lines protocol), `benchmark`.
"""
import json, sys, math

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
    sx, sy = q.get("boardXSize", 19), q.get("boardYSize", 19)
    moves = q.get("moves", [])
    n = sx * sy
    for turn in q.get("analyzeTurns", [len(moves)]):
        to_move = "B" if (turn % 2 == 0) else "W"
        wr = 0.5 + 0.3 * math.sin(turn / 3.0)
        lead = (wr - 0.5) * 30
        cands = []
        for i, mv in enumerate(cand_moves(sx, sy, turn)):
            cands.append({"move": mv, "visits": max(1, 100 - 30 * i), "winrate": max(0.01, min(0.99, wr - 0.02 * i)),
                          "scoreLead": lead - 1.5 * i, "scoreMean": lead - 1.5 * i, "prior": 0.4 / (i + 1), "order": i,
                          "pv": cand_moves(sx, sy, turn + 1 + i)[:4]})
        if turn < len(moves):
            played = moves[turn][1]
            if played not in [c["move"] for c in cands] and played != "pass":
                cands.append({"move": played, "visits": 2, "winrate": max(0.01, wr - 0.2), "scoreLead": lead - 6, "scoreMean": lead - 6,
                              "prior": 0.01, "order": len(cands), "pv": [played]})
        ownership = [round(math.tanh((i % sx - sx / 2) / 4.0), 3) for i in range(n)]
        policy = [round(1.0 / n, 4)] * n + [0.0001]
        human = [round(1.0 / n, 4)] * n + [0.0001]
        resp = {"id": q["id"], "turnNumber": turn, "isDuringSearch": False,
                "rootInfo": {"currentPlayer": to_move, "winrate": wr, "scoreLead": lead, "scoreSelfplay": lead, "scoreStdev": 8.0, "visits": sum(c["visits"] for c in cands), "utility": 0.0},
                "moveInfos": cands, "ownership": ownership, "policy": policy}
        if (q.get("overrideSettings") or {}).get("humanSLProfile"):
            resp["humanPolicy"] = human
        sys.stdout.write(json.dumps(resp) + "\n"); sys.stdout.flush()
