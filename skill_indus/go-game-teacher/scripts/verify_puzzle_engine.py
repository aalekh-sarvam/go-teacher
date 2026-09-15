#!/usr/bin/env python3
"""Search a newly authored puzzle with the available local KataGo, using Black-view scores.
Usage: verify_puzzle_engine.py puzzle.json engine-check.json --katago PATH --model PATH --config PATH
Finite search support, not a mathematical proof of unique optimality or life/death.
"""
import argparse
import hashlib
import json
from pathlib import Path
import queue
import subprocess
import threading
from go_rules import Position


def position_spec(puzzle):
    return {k:puzzle.get(k,default) for k,default in (
        ('board_size',9),('black_stones',[]),('white_stones',[]),('player_to_move','B'),('rules','japanese'),('komi',6.5))}


def fingerprint(puzzle):
    spec=position_spec(puzzle)
    for key in ('black_stones','white_stones'): spec[key]=sorted(spec[key])
    return hashlib.sha256(json.dumps(spec,sort_keys=True).encode()).hexdigest()


def verify(puzzle, katago, model, config, visits=1000, timeout=300):
    if visits<1: raise ValueError('visits must be positive')
    spec=position_spec(puzzle)
    p=Position(spec['board_size'],black=spec['black_stones'],white=spec['white_stones'],to_move=spec['player_to_move'],rules=spec['rules'])
    choices=list(dict.fromkeys(puzzle['correct_moves']+[w['move'] for w in puzzle.get('wrong_moves',[])]))
    for mv in choices:p.clone().play(mv)
    command=[katago,'analysis','-model',model,'-config',config,'-override-config','reportAnalysisWinratesAs=BLACK']
    # stderr to a file avoids blocking the stdout reader on engine startup logging.
    import tempfile
    with tempfile.TemporaryFile(mode='w+') as log:
        process=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True)
        responses=queue.Queue()
        def read():
            for line in process.stdout: responses.put(line)
            responses.put(None)
        thread=threading.Thread(target=read,daemon=True);thread.start()
        result={}
        try:
            for mv in [None]+choices:
                key=mv or 'root'
                request={'id':key,'initialStones':[['B',m] for m in spec['black_stones']]+[['W',m] for m in spec['white_stones']],
                    'initialPlayer':spec['player_to_move'],'moves':[],'rules':spec['rules'],'komi':spec['komi'],
                    'boardXSize':spec['board_size'],'boardYSize':spec['board_size'],'maxVisits':visits,'analysisPVLen':30}
                if mv:request['allowMoves']=[{'player':spec['player_to_move'],'moves':[mv],'untilDepth':1}]
                process.stdin.write(json.dumps(request)+'\n');process.stdin.flush()
                while True:
                    line=responses.get(timeout=timeout)
                    if line is None:
                        log.seek(0);raise ValueError('KataGo exited: '+log.read()[-2000:])
                    value=json.loads(line)
                    if value.get('id')!=key:continue
                    if value.get('error'):raise ValueError(value['error'])
                    if value.get('isDuringSearch'):continue
                    result[key]=value;break
        finally:
            if process.poll() is None: process.terminate()
            try:process.wait(timeout=10)
            except subprocess.TimeoutExpired:process.kill();process.wait()
            thread.join(timeout=2)
    return {'puzzle_fingerprint':fingerprint(puzzle),'position':spec,'score_perspective':'Black',
            'engine':str(Path(katago).resolve()),'model':str(Path(model).resolve()),'config':str(Path(config).resolve()),
            'visits_requested':visits,'responses':result,
            'limit':'Finite search of the root and offered choices. Read strongest defences; unseen first moves are not certified wrong.'}


def check_artifact(puzzle, path):
    with open(path) as f:data=json.load(f)
    if data.get('puzzle_fingerprint')!=fingerprint(puzzle) or data.get('score_perspective')!='Black':
        return ['engine artifact belongs to another position, rules, komi or score perspective']
    responses=data.get('responses',{}); root=responses.get('root',{}).get('moveInfos',[])
    errors=[];answers=puzzle['correct_moves']
    if not root or root[0]['move'] not in answers:errors.append('engine root preference is not among the accepted answers')
    values={}
    for mv in answers+[w['move'] for w in puzzle.get('wrong_moves',[])]:
        r=responses.get(mv,{})
        if r.get('error') or not r.get('rootInfo',{}).get('visits'):
            errors.append(f'engine artifact has no completed search for {mv}');continue
        values[mv]=r['rootInfo']['scoreLead']*(1 if puzzle.get('player_to_move','B')=='B' else -1)
    best=max((values[m] for m in answers if m in values),default=None)
    if best is not None:
        for mv in answers:
            if mv in values and best-values[mv]>0.5:
                errors.append(f'{mv}: accepted answer is materially worse in this search; revise or explain a narrower objective')
        for w in puzzle.get('wrong_moves',[]):
            if w['move'] in values and best-values[w['move']]<0.5:
                errors.append(f'{w["move"]}: search does not clearly distinguish this wrong choice; accept it or redesign')
    return errors


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('puzzle');ap.add_argument('output')
    for name in ('katago','model','config'):ap.add_argument('--'+name,required=True)
    ap.add_argument('--visits',type=int,default=1000);args=ap.parse_args()
    with open(args.puzzle) as f:puzzle=json.load(f)
    result=verify(puzzle,args.katago,args.model,args.config,args.visits)
    with open(args.output,'w') as f:json.dump(result,f,indent=1)
    print('Saved finite-search evidence. Inspect PVs and run the lesson validator; this is not a uniqueness proof.')

if __name__=='__main__':main()
