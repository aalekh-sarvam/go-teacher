// One perspective for the page, one evidence panel and sequence cursor for each lesson.
const lessonControllers = [];
let lessonPerspective = 'engine';
function setPerspective(level) {
    lessonPerspective = level;
    document.querySelectorAll('[data-perspective]').forEach(b => {
        const active = b.dataset.perspective === level;
        b.setAttribute('aria-pressed', String(active)); b.classList.toggle('active',active);
    });
    lessonControllers.forEach(c => c.perspectiveChanged());
}
function initLesson(idx, data, allMoves) {
    const size = data.board_size || 9, prefix = data.move_number-1;
    const canvas = document.getElementById('board-'+idx);
    const ev = data.evidence || {}, sequences = ev.sequences || [];
    const base = new GoBoard(size); replayMoves(base,allMoves,prefix);
    const renderer = new Renderer(canvas,base);
    const section = document.getElementById('lesson-'+idx);
    const explain = document.getElementById('explanation-'+idx);
    const details = document.getElementById('facts-'+idx);
    const stateEl = document.getElementById('sequence-state-'+idx);
    const range = document.getElementById('sequence-range-'+idx);
    const selector = document.getElementById('branch-'+idx);
    let panel='position', step=0, current=null, timer=null;
    const score = n => typeof n === 'number' ? (n >= 0 ? 'B+' : 'W+') + Math.abs(n).toFixed(1) : 'unavailable';
    const signName = data.player_color === 'W' ? 'White' : 'Black';
    function fallback(id,moves) { return {id,from_turn:prefix,first_to_move:data.player_color,moves:moves||[],source:'engine principal variation',stop_reason:'line end'}; }
    function find(id) { return sequences.find(s=>s.id===id); }
    function choices() {
        const level=lessonPerspective;
        if(panel==='position') return [];
        if(panel==='alternatives') {
            if(level!=='engine') return [];
            const tree=sequences.filter(s=>s.id.startsWith('tree_'));
            return tree.length?tree:(data.alternatives||[]).map(a=>fallback('alternative_'+a.move,[a.move]));
        }
        if(panel==='local') return level==='engine' ? ((ev.local_reading||{}).trials||[]).filter(t=>t.sequence).map(t=>t.sequence) : [];
        const kind=panel==='played'?'played':'best';
        if(panel==='what_if') {
            const picks=[find('played_'+level),find('best_'+level)].filter(Boolean);
            return picks;
        }
        let seq=find(kind+'_'+level);
        if(!seq && level==='engine') seq=kind==='played'?fallback('played_engine',[data.played_move,...(data.refutation||[])]):fallback('best_engine',data.better_line);
        return seq ? (panel==='played'&&level!=='engine'&&find('opponent_refutation')?[seq,find('opponent_refutation')]:[seq]) : [];
    }
    function prose(text) {
        explain.replaceChildren();
        for(const paragraph of (text||'').split(/\n\s*\n/)) {
            const p=document.createElement('p');p.textContent=paragraph;explain.appendChild(p);
        }
    }
    function fact(text) {const p=document.createElement('p');p.textContent=text;details.appendChild(p);}
    function stop() {if(timer){clearInterval(timer);timer=null;}document.getElementById('sequence-play-'+idx).textContent='Play';}
    function render() {
        let b=new GoBoard(size), marks=[];
        replayMoves(b,allMoves,current?current.from_turn:prefix);
        let color=current ? (['B','Black'].includes(current.first_to_move)?1:2) : 1;
        let passes=[];
        if(current) current.moves.slice(0,step).forEach((mv,k)=>{
            const xy=gtpToXY(mv,size);
            if(xy){b.play(xy[0],xy[1],color);marks=marks.filter(m=>m.x!==xy[0]||m.y!==xy[1]);marks.push({x:xy[0],y:xy[1],type:'label',color:color===1?'#fff':'#111',text:String(k+1),stone:color});}
            else passes.push(`${k+1}: ${color===1?'Black':'White'} passes`);
            color=3-color;
        });
        // Captured numbered stones must disappear; numbers on surviving stones keep ply order.
        marks=marks.filter(m=>b.grid[m.y][m.x]===m.stone);
        if(!current && prefix>0){const prev=allMoves[prefix-1];const xy=gtpToXY(prev.move,size);if(xy)marks.push({x:xy[0],y:xy[1],type:'square',color:'#245d77'});}
        if(panel==='alternatives'&&step===0){
            for(const a of data.alternatives||[]){const xy=gtpToXY(a.move,size);if(xy && !b.grid[xy[1]][xy[0]])marks.push({x:xy[0],y:xy[1],type:'circle',color:qualityColor(a.quality,a.loss_vs_best)});}
        }
        renderer.board=b;renderer.markers=marks;renderer.draw();
        const max=current?current.moves.length:0;
        range.max=max;range.value=step;range.disabled=!max;
        for(const id of ['start','prev','play','next','end'])document.getElementById('sequence-'+id+'-'+idx).disabled=!max;
        stateEl.textContent=current ? `${step} / ${max} moves${passes.length?' · '+passes.join('; '):''}` : 'Position before move '+data.move_number;
    }
    function activate(key,keepSelection=false) {
        stop();panel=key;step=0;
        section.querySelectorAll('[data-panel]').forEach(b=>{const active=b.dataset.panel===key;b.classList.toggle('active',active);b.setAttribute('aria-expanded',String(active));});
        const options=choices(),previous=selector.value;
        selector.replaceChildren();
        options.forEach(s=>{const o=document.createElement('option');o.value=s.id;o.textContent=s.id.startsWith('alternative_')?s.moves[0]:s.id==='opponent_refutation'?'Most likely opponent reply':s.id.startsWith('tree_')?s.moves.slice(0,3).join(' → '):s.id.startsWith('played')?'After your move':s.id.startsWith('best')?'After the better move':s.id.replaceAll('_',' ');selector.appendChild(o);});
        if(keepSelection && options.some(s=>s.id===previous))selector.value=previous;
        selector.hidden=options.length<2;current=options.find(s=>s.id===selector.value)||options[0]||null;
        prose((data.panels||{})[panel] || (panel==='played'?data.explanation:panel==='best'?data.variation_explanation:panel==='position'?'Start with the position. Choose a panel to explore a consequence or a different plan.':''));
        details.replaceChildren();
        if(panel!=='position'&&!current)fact('Unavailable in this perspective. '+(panel==='local'?'Local reading is a restricted engine search.':panel==='alternatives'?'These branches were evaluated by the engine; human examples are in the played and better-plan panels.':Object.values(ev.unavailable||{}).join('; ')));
        if(current){
            fact(current.source+(current.profile?' · '+current.profile:''));
            if(current.score_black!==undefined && current.score_black!==null)fact((current.source.includes('principal')?'Search estimate: ':'Endpoint estimate: ')+score(current.score_black)+' · '+current.visits+' visits');
            if(current.stop_reason)fact('Line ends: '+current.stop_reason);
            if(current.profile)fact('One human-style example; later choices can change the result.');
        }
        if(panel==='position'){
            const p=ev.pass_comparison||{};if(typeof p.best_vs_pass_points==='number')fact(`Best play versus passing: ${p.best_vs_pass_points.toFixed(1)} points for ${signName}. Whole-board estimate.`);
            if(data.difficulty.label)fact(data.difficulty.label+' · '+data.difficulty.near_best_searched+' near-best searched moves.');
        }
        if(panel==='played'||panel==='best'){
            const mv=panel==='played'?data.played_move:data.preferred_move;
            const v=(ev.initiative||[]).find(v=>v.move===mv);if(v)fact(`${mv}: ${v.class}; reply ${v.reply}. Estimate, not proof of a forcing threat.`);
            if(panel==='best')for(const r of ((ev.ownership_plan||{}).regions||[]).filter(r=>Math.abs(r.cost_of_played)>=1.0))fact(`${r.region}: predicted ownership difference ${r.cost_of_played.toFixed(1)} for ${signName} (better minus played).`);
        }
        if(panel==='local'){
            const l=ev.local_reading||{};
            if(l.stones)fact(`${l.group_color}: ${l.stones.join(', ')} · ${l.liberties} liberties before the move.`);
            for(const t of l.trials||[])fact(`${t.first} first: ${t.prediction||t.reason||'unavailable'}${t.move?' · '+t.move:''}.`);
            fact(l.caveat||l.reason||'No local reading available.');
        }
        if(panel==='what_if')for(const c of ev.rollout_comparisons||[])if(c.level===lessonPerspective){fact('Best-start minus played-start endpoint: '+c.best_minus_played_for_student+' points for '+signName+'.');fact(c.caveat);}
        if(panel==='alternatives'&&current){const alt=(data.alternatives||[]).find(a=>a.move===current.moves[0]);const text=(data.alternative_explanations||{})[current.moves[0]]||(alt||{}).explanation;if(text)prose(text);if(alt&&typeof alt.loss_vs_best==='number')fact(alt.move+': '+alt.loss_vs_best.toFixed(1)+' points worse than the main search’s preferred move for '+signName+'.');}
        step=current ? Math.min(current.moves.length,panel==='what_if'?12:6) : 0;
        render();
    }
    section.querySelectorAll('[data-panel]').forEach(b=>b.onclick=()=>activate(b.dataset.panel));
    selector.onchange=()=>activate(panel,true);
    range.oninput=()=>{stop();step=Number(range.value);render();};
    for(const [id,action] of [['start',()=>0],['prev',()=>Math.max(0,step-1)],['next',()=>Math.min(current.moves.length,step+1)],['end',()=>current.moves.length]]){
        document.getElementById('sequence-'+id+'-'+idx).onclick=()=>{if(!current)return;stop();step=action();render();};
    }
    document.getElementById('sequence-play-'+idx).onclick=()=>{
        if(timer){stop();return;}if(!current)return;if(step===current.moves.length)step=0;
        document.getElementById('sequence-play-'+idx).textContent='Pause';
        timer=setInterval(()=>{step++;render();if(step>=current.moves.length)stop();},750);
    };
    lessonControllers.push({perspectiveChanged:()=>activate(panel)});activate('position');
}
