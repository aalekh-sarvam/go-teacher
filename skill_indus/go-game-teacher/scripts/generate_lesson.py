#!/usr/bin/env python3
"""Generate a self-contained interactive HTML Go lesson from parsed data + lesson content."""

from pathlib import Path
from lesson_contract import hydrate
import json
import sys


# JavaScript Go board engine + renderer (embedded in the HTML)
GO_BOARD_JS = r'''
// ===== Go Board Engine =====
const COLS = "ABCDEFGHJKLMNOPQRST";

function gtpToXY(gtp, size) {
    if (gtp === 'pass') return null;
    const x = COLS.indexOf(gtp[0]);
    const y = size - parseInt(gtp.slice(1));
    return [x, y];
}

function xyToGtp(x, y, size) {
    return COLS[x] + (size - y);
}

class GoBoard {
    constructor(size) {
        this.size = size;
        this.grid = Array(size).fill(null).map(() => Array(size).fill(0));
    }
    clone() {
        const b = new GoBoard(this.size);
        for (let y = 0; y < this.size; y++)
            for (let x = 0; x < this.size; x++)
                b.grid[y][x] = this.grid[y][x];
        return b;
    }
    play(x, y, color) {
        if (x < 0 || x >= this.size || y < 0 || y >= this.size || this.grid[y][x]) return false;
        this.grid[y][x] = color;
        const opp = color === 1 ? 2 : 1;
        for (const [dx, dy] of [[0,1],[0,-1],[1,0],[-1,0]]) {
            const nx = x + dx, ny = y + dy;
            if (nx>=0 && nx<this.size && ny>=0 && ny<this.size && this.grid[ny][nx] === opp) {
                const group = this.getGroup(nx, ny);
                if (this.liberties(group) === 0) {
                    for (const [gx, gy] of group) this.grid[gy][gx] = 0;
                }
            }
        }
        const own = this.getGroup(x,y);
        if(this.liberties(own)===0)for(const [gx,gy] of own)this.grid[gy][gx]=0;
        return true;
    }
    placeStones(black, white) {
        for (const s of black || []) {
            const [x, y] = gtpToXY(s, this.size);
            if (x !== null) this.grid[y][x] = 1;
        }
        for (const s of white || []) {
            const [x, y] = gtpToXY(s, this.size);
            if (x !== null) this.grid[y][x] = 2;
        }
    }
    getGroup(x, y) {
        const color = this.grid[y][x];
        if (color === 0) return [];
        const seen = new Set(), group = [], stack = [[x, y]];
        while (stack.length) {
            const [cx, cy] = stack.pop();
            const k = cx + "," + cy;
            if (seen.has(k)) continue;
            seen.add(k);
            if (this.grid[cy][cx] === color) {
                group.push([cx, cy]);
                for (const [dx, dy] of [[0,1],[0,-1],[1,0],[-1,0]]) {
                    const nx = cx+dx, ny = cy+dy;
                    if (nx>=0 && nx<this.size && ny>=0 && ny<this.size) stack.push([nx, ny]);
                }
            }
        }
        return group;
    }
    liberties(group) {
        const libs = new Set();
        for (const [x, y] of group) {
            for (const [dx, dy] of [[0,1],[0,-1],[1,0],[-1,0]]) {
                const nx = x+dx, ny = y+dy;
                if (nx>=0 && nx<this.size && ny>=0 && ny<this.size && this.grid[ny][nx] === 0)
                    libs.add(nx + "," + ny);
            }
        }
        return libs.size;
    }
}

// ===== Canvas Renderer =====
class Renderer {
    constructor(canvas, board) {
        this.canvas = canvas;
        this.ctx = canvas.getContext('2d');
        this.board = board;
        this.size = board.size;
        this.markers = [];
        this.computeLayout();
    }
    computeLayout() {
        const w = this.canvas.width;
        this.cell = Math.floor((w - 50) / (this.size - 1));
        this.margin = (w - this.cell * (this.size - 1)) / 2;
    }
    toPx(x, y) { return [this.margin + x * this.cell, this.margin + y * this.cell]; }
    toXY(px, py) {
        const x = Math.round((px - this.margin) / this.cell);
        const y = Math.round((py - this.margin) / this.cell);
        if (x>=0 && x<this.size && y>=0 && y<this.size) return [x, y];
        return null;
    }
    draw() {
        const ctx = this.ctx, cs = this.cell, s = this.size;
        ctx.fillStyle = '#dcb35c';
        ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
        ctx.strokeStyle = '#333'; ctx.lineWidth = 1;
        for (let i = 0; i < s; i++) {
            const [x1,y1] = this.toPx(0,i), [x2,y2] = this.toPx(s-1,i);
            ctx.beginPath(); ctx.moveTo(x1,y1); ctx.lineTo(x2,y2); ctx.stroke();
            const [x3,y3] = this.toPx(i,0), [x4,y4] = this.toPx(i,s-1);
            ctx.beginPath(); ctx.moveTo(x3,y3); ctx.lineTo(x4,y4); ctx.stroke();
        }
        for (const [sx, sy] of this.starPoints()) {
            const [px,py] = this.toPx(sx,sy);
            ctx.fillStyle = '#333'; ctx.beginPath(); ctx.arc(px,py,3,0,7); ctx.fill();
        }
        for (let y=0;y<s;y++) for (let x=0;x<s;x++) {
            if (this.board.grid[y][x] !== 0) this.drawStone(x,y,this.board.grid[y][x]);
        }
        for (const m of this.markers) this.drawMarker(m);
        // Coords
        ctx.fillStyle = '#666'; ctx.font = '11px sans-serif';
        ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
        for (let i=0;i<s;i++) {
            const [px,py] = this.toPx(i,0);
            ctx.fillText(COLS[i], px, py - this.margin*0.6);
            const [px2,py2] = this.toPx(0,i);
            ctx.fillText(String(s-i), px2 - this.margin*0.6, py2);
        }
    }
    drawStone(x, y, color) {
        const [px,py] = this.toPx(x,y), r = this.cell*0.45, ctx = this.ctx;
        ctx.fillStyle = 'rgba(0,0,0,0.25)';
        ctx.beginPath(); ctx.arc(px+1, py+2, r, 0, 7); ctx.fill();
        ctx.fillStyle = color === 1 ? '#1a1a1a' : '#f5f5f0';
        ctx.beginPath(); ctx.arc(px, py, r, 0, 7); ctx.fill();
        ctx.strokeStyle = color === 1 ? '#000' : '#999'; ctx.lineWidth = 1; ctx.stroke();
    }
    drawMarker(m) {
        const [px,py] = this.toPx(m.x,m.y), ctx = this.ctx, r = this.cell*0.28;
        if (m.type === 'circle') {
            ctx.strokeStyle = m.color || '#e44'; ctx.lineWidth = 3;
            ctx.beginPath(); ctx.arc(px,py,r,0,7); ctx.stroke();
        } else if (m.type === 'triangle') {
            ctx.strokeStyle = m.color || '#e44'; ctx.lineWidth = 2.5;
            ctx.beginPath(); ctx.moveTo(px,py-r);
            ctx.lineTo(px-r*0.87,py+r*0.5); ctx.lineTo(px+r*0.87,py+r*0.5);
            ctx.closePath(); ctx.stroke();
        } else if (m.type === 'square') {
            ctx.strokeStyle = m.color || '#24a'; ctx.lineWidth = 2.5;
            ctx.beginPath();
            ctx.rect(px - r*0.8, py - r*0.8, r*1.6, r*1.6);
            ctx.stroke();
        } else if (m.type === 'label') {
            ctx.fillStyle = m.color || '#fff';
            ctx.font = 'bold ' + Math.floor(this.cell*0.4) + 'px sans-serif';
            ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
            ctx.fillText(m.text, px, py);
        }
    }
    starPoints() {
        const s = this.size;
        if (s===9) return [[2,2],[2,6],[6,2],[6,6],[4,4]];
        if (s===13) return [[3,3],[3,9],[9,3],[9,9],[6,6]];
        if (s===19) return [[3,3],[3,9],[3,15],[9,3],[9,9],[9,15],[15,3],[15,9],[15,15]];
        return [];
    }
}

// ===== Quality grading =====
const QUALITY_COLORS = { best: '#2a2', good: '#5cb85c', inaccuracy: '#d4a017', mistake: '#e67e22', 'big mistake': '#d9482b', blunder: '#e44' };
function qualityFromLoss(loss) {
    if (loss === undefined || loss === null) return 'mistake';
    if (loss < 0.5) return 'best';
    if (loss < 1.5) return 'good';
    if (loss < 3) return 'inaccuracy';
    if (loss < 6) return 'mistake';
    if (loss < 12) return 'big mistake';
    return 'blunder';
}
function qualityColor(q, loss) { return QUALITY_COLORS[q || qualityFromLoss(loss)] || '#e44'; }
function qualityLabel(q, loss) {
    const name = q || qualityFromLoss(loss);
    const pts = (loss !== undefined && loss !== null) ? ' (' + (loss < 0.05 ? 'best' : loss.toFixed(1) + ' pts worse than the best move') + ')' : '';
    return name + pts;
}

// ===== Lesson & Puzzle Logic =====
function replayMoves(board, moves, upTo) {
    board.placeStones(gameSetup.black, gameSetup.white);
    for (let i = 0; i < upTo && i < moves.length; i++) {
        const m = moves[i];
        const xy = gtpToXY(m.move, board.size);
        if (xy) board.play(xy[0], xy[1], m.color === 'B' ? 1 : 2);
    }
}

function initGoodMove(idx, data, allMoves) {
    const canvas = document.getElementById('gboard-'+idx);
    const size = data.board_size || 9;
    const board = new GoBoard(size);
    replayMoves(board, allMoves, data.move_number);
    const renderer = new Renderer(canvas, board);

    // Mark the player's good move with a green circle
    const xy = gtpToXY(data.move, size);
    if (xy) renderer.markers = [{x:xy[0], y:xy[1], type:'circle', color:'#2a2'}];

    // Also mark opponent's last move if available
    if (data.move_number >= 2 && allMoves[data.move_number - 2]) {
        const oppMove = allMoves[data.move_number - 2];
        const oppXY = gtpToXY(oppMove.move, size);
        if (oppXY) renderer.markers.unshift({x:oppXY[0], y:oppXY[1], type:'square', color:'#24a'});
    }

    renderer.draw();
}

function htmlText(s) { const e=document.createElement("span");e.textContent=s||"";return e.innerHTML; }
function initPuzzle(idx, data) {
    const canvas = document.getElementById('pboard-'+idx);
    const size = data.board_size || 9;
    const board = new GoBoard(size);
    board.placeStones(data.black_stones, data.white_stones);
    const baseBoard = board.clone();

    // Mark the opponent's last move if provided
    var oppMarker = null;
    if (data.opponent_last_move) {
        const oppXY = gtpToXY(data.opponent_last_move, size);
        if (oppXY) {
            oppMarker = {x: oppXY[0], y: oppXY[1], type: 'square', color: '#24a'};
        }
    }

    const renderer = new Renderer(canvas, board);
    renderer.markers = oppMarker ? [oppMarker] : [];
    renderer.draw();

    const feedback = document.getElementById('pfeedback-'+idx);
    const hintEl = document.getElementById('phint-'+idx);

    canvas.onclick = function(e) {
        const rect = canvas.getBoundingClientRect();
        const scale = canvas.width / rect.width;
        const px = (e.clientX - rect.left) * scale;
        const py = (e.clientY - rect.top) * scale;
        const xy = renderer.toXY(px, py);
        if (!xy || baseBoard.grid[xy[1]][xy[0]]) return;

        // Reset to base
        renderer.board = baseBoard.clone();
        // Place the clicked stone
        const color = data.player_to_move === 'W' ? 2 : 1;
        renderer.board.play(xy[0], xy[1], color);
        var markers = oppMarker ? [oppMarker] : [];
        markers.push({x:xy[0], y:xy[1], type:'circle', color:'#e44'});

        const clickedGtp = xyToGtp(xy[0], xy[1], size);
        if (data.correct_moves.includes(clickedGtp)) {
            markers[markers.length - 1] = {x:xy[0], y:xy[1], type:'circle', color:'#2a2'};
            const solution=(data.correct_lines||{})[clickedGtp]||[];
            let nextColor=3-color;
            solution.slice(1).forEach((mv,k)=>{
                const point=gtpToXY(mv,size);
                if(point){renderer.board.play(point[0],point[1],nextColor);markers.push({x:point[0],y:point[1],type:'label',color:nextColor===1?'#fff':'#111',text:String(k+2),stone:nextColor});}
                nextColor=3-nextColor;
            });
            renderer.markers = markers.filter(m=>!m.stone || renderer.board.grid[m.y][m.x]===m.stone);
            feedback.innerHTML = '<p class="correct">&#10003; Correct! ' + htmlText(data.explanation) + '</p>';
            feedback.className = 'feedback correct';
        } else {
            const wrong = (data.wrong_moves || []).find(w => w.move === clickedGtp);
            if (wrong) {
                const col = qualityColor(wrong.quality, wrong.loss_vs_best);
                markers[markers.length - 1] = {x:xy[0], y:xy[1], type:'circle', color:col};
                // Show the punishment: the opponent's replies to the wrong move, numbered.
                if (wrong.refutation && wrong.refutation.length) {
                    let c = color === 1 ? 2 : 1;
                    wrong.refutation.forEach((mv, k) => {
                        const rxy = gtpToXY(mv, size);
                        if (!rxy) { c = c === 1 ? 2 : 1; return; }
                        renderer.board.play(rxy[0], rxy[1], c);
                        markers.push({x:rxy[0], y:rxy[1], type:'label', color: c === 1 ? '#fff' : '#000', text: String(k + 1), stone:c});
                        c = c === 1 ? 2 : 1;
                    });
                }
                renderer.markers = markers.filter(m=>!m.stone || renderer.board.grid[m.y][m.x]===m.stone);
                feedback.innerHTML = '<p class="wrong"><span class="quality-badge" style="background:' + col + '">' + (wrong.quality || 'mistake') + '</span> '
                    + clickedGtp + ' falls short of this exercise’s goal. ' + htmlText(wrong.explanation) + '</p><p class="hint-text">Try again, or use "Show evaluations".</p>';
                feedback.className = 'feedback wrong';
            } else {
                renderer.markers = markers.filter(m=>!m.stone || renderer.board.grid[m.y][m.x]===m.stone);
                feedback.innerHTML = '<p>' + clickedGtp + ' has not been checked in this exercise. Compare the evaluated choices or read its continuation before judging it.</p>';
                feedback.className = 'feedback';
                renderer.markers[renderer.markers.length-1].color = '#667788';
            }
        }
        renderer.draw();
    };

    const evalBtn = document.getElementById('btn-eval-'+idx);
    if (evalBtn) evalBtn.onclick = function() {
        renderer.board = baseBoard.clone();
        var markers = oppMarker ? [oppMarker] : [];
        for (const mv of data.correct_moves) {
            const xy = gtpToXY(mv, size);
            if (xy) { markers.push({x:xy[0], y:xy[1], type:'circle', color:'#2a2'}); markers.push({x:xy[0], y:xy[1], type:'label', color:'#000', text:'★'}); }
        }
        var rows = '<li><span class="quality-badge" style="background:#2a2">meets the goal</span> <strong>' + data.correct_moves.join(', ') + '</strong> — ' + htmlText(data.explanation) + '</li>';
        for (const w of (data.wrong_moves || [])) {
            const xy = gtpToXY(w.move, size);
            const col = qualityColor(w.quality, w.loss_vs_best);
            if (xy) { markers.push({x:xy[0], y:xy[1], type:'circle', color:col}); markers.push({x:xy[0], y:xy[1], type:'label', color:'#000', text: w.loss_vs_best !== undefined ? '-' + Math.round(w.loss_vs_best) : '?'}); }
            rows += '<li><span class="quality-badge" style="background:' + col + '">' + (w.quality || 'mistake') + '</span> <strong>' + w.move + '</strong> — ' + w.explanation + '</li>';
        }
        renderer.markers = markers;
        renderer.draw();
        feedback.innerHTML = '<p><strong>How the candidate moves compare</strong> (★ = best; numbers are points lost against it):</p><ul class="eval-list">' + rows + '</ul>';
        feedback.className = 'feedback';
    };

    document.getElementById('btn-hint-'+idx).onclick = function() {
        hintEl.style.display = hintEl.style.display === 'none' ? 'block' : 'none';
    };
    document.getElementById('btn-reset-'+idx).onclick = function() {
        renderer.board = baseBoard.clone();
        renderer.markers = oppMarker ? [oppMarker] : [];
        renderer.draw();
        feedback.innerHTML = '';
        feedback.className = 'feedback';
        hintEl.style.display = 'none';
    };
}
'''

GO_BOARD_JS += Path(__file__).with_name('lesson_panels.js').read_text(encoding='utf-8')

CSS = r'''
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: -apple-system, "Segoe UI", Roboto, sans-serif; background: #f5f0e8; color: #2a2a2a; line-height: 1.6; max-width: 1100px; margin: 0 auto; padding: 20px; }
header { text-align: center; padding: 30px 0 20px; border-bottom: 2px solid #dcb35c; margin-bottom: 30px; }
header h1 { font-size: 1.8em; color: #3a2a10; margin-bottom: 8px; }
.game-info { display: flex; gap: 30px; justify-content: center; flex-wrap: wrap; font-size: 0.95em; color: #666; }
.game-info strong { color: #3a2a10; }
section { background: #fff; border-radius: 10px; padding: 24px; margin-bottom: 24px; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
h2 { color: #3a2a10; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid #e0d5c0; }
.lesson-meta, .puzzle-meta { display: flex; gap: 12px; margin-bottom: 16px; flex-wrap: wrap; }
.badge { background: #dcb35c; color: #3a2a10; padding: 3px 12px; border-radius: 12px; font-size: 0.85em; font-weight: 600; }
.badge-theme { background: #e8e0f4; color: #4a3a6a; }
.move-ref { color: #888; font-size: 0.85em; padding: 3px 0; }
.loss { color: #c33; font-size: 0.85em; padding: 3px 0; }
.lesson-content, .puzzle-content { display: flex; gap: 24px; flex-wrap: wrap; }
.board-container { flex: 0 0 auto; }
.board-container canvas { border-radius: 6px; box-shadow: 0 2px 6px rgba(0,0,0,0.15); cursor: crosshair; max-width: 100%; height: auto; }
.lesson-controls { display: flex; gap: 8px; margin-top: 10px; flex-wrap: wrap; }
.lesson-controls button, .puzzle-controls button { padding: 6px 14px; border: 1px solid #c0a060; background: #f5edd5; color: #3a2a10; border-radius: 6px; cursor: pointer; font-size: 0.9em; transition: all 0.15s; }
.lesson-controls button:hover, .puzzle-controls button:hover { background: #e8d8a8; }
.lesson-controls button.active { background: #dcb35c; font-weight: 600; }
.board-legend { display: flex; gap: 16px; margin-top: 8px; font-size: 0.8em; color: #666; flex-wrap: wrap; }
.legend-item { display: flex; align-items: center; gap: 5px; }
.legend-swatch { width: 14px; height: 14px; display: inline-block; }
.legend-square { border: 2px solid #24a; }
.legend-circle-red { border: 3px solid #e44; border-radius: 50%; }
.legend-circle-green { border: 3px solid #2a2; border-radius: 50%; }
.good-move-card { display: flex; gap: 20px; align-items: flex-start; margin-bottom: 16px; padding: 14px; background: #f6faf6; border-radius: 8px; border-left: 4px solid #2a2; }
.good-move-card .move-info { flex: 1; }
.rating-badge { display: inline-block; background: #2a2; color: #fff; padding: 3px 10px; border-radius: 12px; font-size: 0.85em; font-weight: 700; margin-left: 8px; }
.concept-card { margin-bottom: 20px; padding: 16px; background: #f8f5ff; border-radius: 8px; border-left: 4px solid #646; }
.concept-card h3 { color: #3a2a10; margin-bottom: 6px; }
.concept-japanese { color: #646; font-size: 0.95em; font-style: italic; margin-bottom: 8px; }
.concept-resources { margin: 10px 0; padding-left: 4px; }
.concept-resources li { margin-bottom: 4px; list-style: none; }
.concept-resources a { color: #24a; text-decoration: none; }
.concept-resources a:hover { text-decoration: underline; }
.anecdote-box { margin-top: 12px; padding: 12px 16px; background: #fff8e0; border-radius: 6px; border: 1px dashed #c0a060; }
.anecdote-box .anecdote-label { font-weight: 700; color: #864; margin-bottom: 4px; display: block; font-size: 0.85em; }
.lesson-side { flex: 1; min-width: 280px; }
.explanation { margin-top: 14px; }
.explanation p { margin-bottom: 10px; }
.hint-text { color: #888; font-style: italic; }
.principle { margin-top: 16px; padding: 12px 16px; background: #f0e8d0; border-left: 4px solid #dcb35c; border-radius: 4px; }
.principle strong { color: #3a2a10; }
.played-explanation { border-left: 3px solid #e44; padding-left: 12px; }
.preferred-explanation { border-left: 3px solid #2a2; padding-left: 12px; }
.puzzle-instructions { margin: 10px 0; font-weight: 500; }
.feedback { margin-top: 12px; padding: 10px 14px; border-radius: 6px; min-height: 20px; }
.feedback.correct { background: #e8f5e9; border: 1px solid #4caf50; }
.feedback.wrong { background: #fbe9e9; border: 1px solid #e44; }
.feedback p { margin: 0; }
.feedback .correct { color: #2a7; }
.feedback .wrong { color: #c33; }
.hint-box { margin-top: 10px; padding: 10px 14px; background: #fff8e0; border-radius: 6px; border: 1px dashed #c0a060; }
footer { text-align: center; padding: 20px; color: #999; font-size: 0.85em; }
.quality-badge { display: inline-block; color: #fff; padding: 1px 8px; border-radius: 10px; font-size: 0.8em; font-weight: 700; }
.alt-controls { display: flex; gap: 6px; margin-top: 8px; flex-wrap: wrap; align-items: center; font-size: 0.85em; color: #666; }
.alt-controls button { padding: 4px 10px; border: 2px solid #c0a060; background: #fff; border-radius: 6px; cursor: pointer; font-size: 0.85em; }
.alt-controls button.active { background: #f0e8d0; font-weight: 600; }
.eval-list { margin: 8px 0 0 18px; }
.eval-list li { margin-bottom: 6px; }
.legend-circle-yellow { border: 3px solid #d4a017; border-radius: 50%; }
.arc-section { }
.arc-intro { color: #555; font-style: italic; margin-bottom: 14px; }
.phase-card { margin-bottom: 16px; padding: 14px 16px; background: #faf6ee; border-radius: 8px; border-left: 4px solid #b88a2a; }
.phase-header { display: flex; gap: 10px; align-items: baseline; margin-bottom: 8px; flex-wrap: wrap; }
.phase-name { font-weight: 700; color: #3a2a10; font-size: 1.05em; }
.phase-range { color: #8a6a2a; font-size: 0.85em; }
.phase-card p { margin-bottom: 8px; }
.phase-card p:last-child { margin-bottom: 0; }
.turning-points { margin: 8px 0 0 18px; }
.turning-points li { margin-bottom: 4px; font-size: 0.92em; color: #555; }
.lesson-story { margin-bottom: 14px; padding: 10px 14px; background: #f0f4f8; border-radius: 6px; border-left: 3px solid #5a7a9a; }
.level-framing { margin-top: 12px; padding: 10px 14px; background: #f6f0fa; border-radius: 6px; border-left: 3px solid #8a6aa8; }
.level-framing .story-label { font-weight: 700; color: #4a3a6a; display: block; margin-bottom: 4px; font-size: 0.85em; text-transform: uppercase; letter-spacing: 0.03em; }
.progress-table { border-collapse: collapse; margin-top: 10px; }
.progress-table th, .progress-table td { border: 1px solid #e0d5c0; padding: 5px 10px; text-align: left; font-size: 0.92em; }
.progress-table th { background: #f5edd5; }
.lesson-story .story-label { font-weight: 700; color: #3a4a5a; display: block; margin-bottom: 4px; font-size: 0.85em; text-transform: uppercase; letter-spacing: 0.03em; }
.overview-text p { margin-bottom: 12px; }
@media (max-width: 700px) {
    .lesson-content, .puzzle-content { flex-direction: column; }
    .board-container canvas { width: 100%; }
}
'''


def escape_html(text):
    """Escape HTML special characters, converting **bold** markdown to <strong>."""
    # Replace ** with placeholder before escaping
    text = text.replace('**', '\x00B\x00')
    # Escape HTML special chars
    text = text.replace('&', chr(38)+'amp;')
    text = text.replace('<', chr(38)+'lt;')
    text = text.replace('>', chr(38)+'gt;')
    # Convert bold placeholders to <strong></strong> pairs
    if '\x00B\x00' in text:
        parts = text.split('\x00B\x00')
        result = ''
        for i, part in enumerate(parts):
            result += part
            if i < len(parts) - 1:
                result += '<strong>' if i % 2 == 0 else '</strong>'
        return result
    return text


def format_paragraphs(text):
    """Convert plain text paragraphs to HTML."""
    paragraphs = text.strip().split('\n\n')
    html_parts = []
    for p in paragraphs:
        p = p.strip()
        if p:
            escaped = escape_html(p)
            # Convert single newlines to <br>
            escaped = escaped.replace('\n', '<br>')
            html_parts.append(f'<p>{escaped}</p>')
    return '\n'.join(html_parts)


def build_arc_section(game_arc):
    """Build the 'How the Game Unfolded' section HTML, or '' if no arc data."""
    if not game_arc or not game_arc.get('phases'):
        return ''
    intro_html = ''
    if game_arc.get('intro'):
        intro_html = f'<p class="arc-intro">{escape_html(game_arc["intro"])}</p>'
    cards = ''
    for ph in game_arc.get('phases', []):
        name = escape_html(str(ph.get('phase', '')))
        rng = escape_html(str(ph.get('move_range', ''))) if ph.get('move_range') else ''
        header = f'<div class="phase-header"><span class="phase-name">{name}</span>'
        if rng:
            header += f'<span class="phase-range">moves {rng}</span>'
        header += '</div>'
        body = format_paragraphs(ph.get('narrative', ''))
        tp_html = ''
        turning = ph.get('turning_points') or []
        if turning:
            items = ''.join(f'<li>{escape_html(str(t))}</li>' for t in turning)
            tp_html = f'<ul class="turning-points">{items}</ul>'
        cards += f'<div class="phase-card">{header}{body}{tp_html}</div>'
    return ('<section class="arc-section">'
            '<h2>How the Game Unfolded</h2>'
            f'{intro_html}{cards}</section>')


def build_progress_section(progress):
    """'Your progress' card from lesson_data['progress'] (text + optional rows), or ''."""
    if not progress or not (progress.get('summary') or progress.get('rows')):
        return ''
    rows = ''
    for r in progress.get('rows', []):
        rows += f"<tr><td>{escape_html(str(r.get('metric', '')))}</td><td>{escape_html(str(r.get('this_game', '')))}</td><td>{escape_html(str(r.get('recent_average', '')))}</td></tr>"
    table = f'<table class="progress-table"><thead><tr><th>Metric</th><th>This game</th><th>Recent average</th></tr></thead><tbody>{rows}</tbody></table>' if rows else ''
    return ('<section class="progress"><h2>Your Progress</h2>'
            + format_paragraphs(progress.get('summary', '')) + table + '</section>')


def build_story_html(lesson):
    """Build the 'How we got here' block for a lesson, or '' if no story."""
    if not lesson.get('story'):
        return ''
    return ('<div class="lesson-story"><span class="story-label">How we got here</span>'
            f'{format_paragraphs(lesson["story"])}</div>')


def generate_html(parsed_data, lesson_data):
    """Generate the complete HTML file."""
    lesson_data = hydrate(parsed_data, lesson_data)
    gi = parsed_data.get('game_info', {})
    moves = parsed_data.get('moves', [])

    # Colour of the student for each lesson: explicit in the lesson, else the colour of that move
    # in the parsed move list, else the parsed student, else Black.
    default_color = gi.get('student', 'B')
    moves_by_number = {m['number']: m for m in moves}

    def color_for(lesson):
        c = lesson.get('player_color')
        if c in ('B', 'W'):
            return c
        m = moves_by_number.get(lesson.get('move_number'))
        if m and m.get('color') in ('B', 'W'):
            return m['color']
        return default_color

    # Build lesson sections
    lesson_sections = []
    for i, lesson in enumerate(lesson_data.get('lessons', [])):
        move_num = lesson['move_number']
        move_detail = parsed_data.get('move_details', {}).get(str(move_num), {})

        player_color = color_for(lesson)
        alternatives = lesson.get('alternatives', [])
        story_html = build_story_html(lesson)

        winrate_info = ''
        if 'winrate_before' in lesson:
            winrate_info = (f'<span class="move-ref">Winrate: {lesson["winrate_before"]} '
                          f'→ {lesson["winrate_after"]}</span>')
        elif 'winrate_before' in move_detail:
            winrate_info = (f'<span class="move-ref">Winrate: {move_detail["winrate_before"]} '
                          f'→ {move_detail["winrate_after"]}</span>')

        score_info = ''
        if 'score_before' in lesson:
            score_info = f'<span class="move-ref">Score: {lesson["score_before"]} → {lesson["score_after"]}</span>'
        elif 'score_before' in move_detail:
            score_info = f'<span class="move-ref">Score: {move_detail["score_before"]} → {move_detail["score_after"]}</span>'

        alt_buttons = ''
        if alternatives:
            alt_buttons = '<div class="alt-controls"><span>Other moves you might consider:</span>' + ''.join(
                f'<button id="btn-alt-{i}-{k}">{escape_html(a["move"])}'
                + (f' ({a["loss_vs_best"]:+.1f})'.replace('+', '-') if isinstance(a.get('loss_vs_best'), (int, float)) and a['loss_vs_best'] >= 0.05 else '')
                + '</button>'
                for k, a in enumerate(alternatives)) + '</div>'
        section = f'''
<section class="lesson" id="lesson-{i}">
  <h2>Lesson {i+1}: {escape_html(lesson['title'])}</h2>
  <div class="lesson-meta">
    <span class="badge">{escape_html(lesson['concept_label'])}</span>
    {('<span class="badge badge-theme">' + escape_html(lesson['theme']) + '</span>') if lesson.get('theme') else ''}
    <span class="move-ref">Move {move_num}</span>
    <span class="loss">Loss: {float(lesson['point_loss']):.1f} pts</span>
    {winrate_info}
    {score_info}
  </div>
  <div class="lesson-content">
    <div class="board-container">
      <canvas id="board-{i}" width="500" height="500"></canvas>
      <div class="sequence-controls" aria-label="Sequence playback">
        <button id="sequence-start-{i}" aria-label="First position">|◀</button>
        <button id="sequence-prev-{i}" aria-label="Previous move">◀</button>
        <button id="sequence-play-{i}">Play</button>
        <button id="sequence-next-{i}" aria-label="Next move">▶</button>
        <button id="sequence-end-{i}" aria-label="Last position">▶|</button>
      </div>
      <input class="sequence-range" id="sequence-range-{i}" type="range" min="0" max="0" value="0" aria-label="Move in selected sequence">
      <p id="sequence-state-{i}" class="sequence-state" aria-live="polite"></p>
      <div class="board-legend">
        <span class="legend-item"><span class="legend-swatch legend-square"></span> Opponent's last move</span>
        <span class="legend-item"><span class="legend-swatch legend-circle-red"></span> Your move</span>
        <span class="legend-item"><span class="legend-swatch legend-circle-green"></span> KataGo's recommendation</span>
        <span class="legend-item"><span class="legend-swatch legend-circle-yellow"></span> Other candidates, coloured by loss</span>
      </div>
    </div>
    <div class="lesson-side">
      {story_html}
      <div class="evidence-panels" aria-label="Explore this lesson">
        <button data-panel="position" aria-expanded="true"><strong>The position</strong><span>What needs attention?</span></button>
        <button data-panel="played" aria-expanded="false"><strong>Your move</strong><span>Follow what it allows</span></button>
        <button data-panel="best" aria-expanded="false"><strong>A better plan</strong><span>See how the idea works</span></button>
        <button data-panel="alternatives" aria-expanded="false"><strong>Other choices</strong><span>Compare evaluated branches</span></button>
        <button data-panel="local" aria-expanded="false"><strong>Read the local fight</strong><span>Explore the restricted search</span></button>
        <button data-panel="what_if" aria-expanded="false"><strong>What happens next?</strong><span>Follow a longer example</span></button>
      </div>
      <select class="branch-choice" id="branch-{i}" aria-label="Variation branch" hidden></select>
      <div class="explanation" id="explanation-{i}">
        <p class="hint-text">Click "Show played move" to see what you played, or "Show better move" to see KataGo's recommendation.</p>
      </div>
      <div class="evidence-facts" id="facts-{i}"></div>
      {('<div class="level-framing"><span class="story-label">At your level</span>' + format_paragraphs(lesson['level_framing']) + '</div>') if lesson.get('level_framing') else ''}
      <div class="principle">
        <strong>Key principle:</strong> {escape_html(lesson['principle'])}
      </div>
    </div>
  </div>
</section>'''
        lesson_sections.append(section)

    # Build good moves section
    good_moves_html = ''
    good_moves_js_list = []
    for i, gm in enumerate(lesson_data.get('good_moves', [])):
        gm_board_size = gi.get('board_size', 9)
        good_moves_js_list.append({
            'move_number': gm['move_number'],
            'move': gm['move'],
            'board_size': gm_board_size,
        })
        rating = (' <span class="rating-badge">' + escape_html(gm['rating']) + '</span>') if gm.get('rating') and gm.get('rating_source') else ''
        gm_card = f'''
<div class="good-move-card">
  <div class="board-container">
    <canvas id="gboard-{i}" width="280" height="280"></canvas>
  </div>
  <div class="move-info">
    <h3>Move {gm['move_number']}: {escape_html(gm['move'])} {rating}</h3>
    <p>{escape_html(gm['explanation'])}</p>
  </div>
</div>'''
        good_moves_html += gm_card

    good_moves_section = ''
    if good_moves_html:
        good_moves_section = f'''
<section class="good-moves">
  <h2>Moves You Played Well</h2>
  <p style="margin-bottom:16px;color:#666;">Here are choices worth repeating, with the evidence for what you did well.</p>
  <div class="board-legend">
    <span class="legend-item"><span class="legend-swatch legend-square"></span> Opponent's last move</span>
    <span class="legend-item"><span class="legend-swatch legend-circle-green"></span> Your good move</span>
  </div>
  {good_moves_html}
</section>'''

    # Build puzzle sections
    puzzle_sections = []
    for i, puzzle in enumerate(lesson_data.get('puzzles', [])):
        player_text = 'Black' if puzzle['player_to_move'] == 'B' else 'White'
        section = f'''
<section class="puzzle" id="puzzle-{i}">
  <h2>Puzzle {i+1}: {escape_html(puzzle['title'])}</h2>
  <div class="puzzle-meta">
    <span class="badge">{escape_html(puzzle['concept_label'])}</span>
  </div>
  <div class="puzzle-content">
    <div class="board-container">
      <canvas id="pboard-{i}" width="500" height="500"></canvas>
      <div class="puzzle-controls">
        <button id="btn-hint-{i}">Hint</button>
        <button id="btn-eval-{i}">Show evaluations</button>
        <button id="btn-reset-{i}">Reset</button>
      </div>
      <div class="board-legend">
        <span class="legend-item"><span class="legend-swatch legend-square"></span> Opponent's last move</span>
        <span class="legend-item"><span class="legend-swatch legend-circle-green"></span> Correct move</span>
        <span class="legend-item"><span class="legend-swatch legend-circle-yellow"></span> Tempting wrong moves, coloured by how bad</span>
      </div>
    </div>
    <div class="lesson-side">
      <p class="puzzle-instructions">{player_text} to play. Find a move that meets the goal — click on the board!</p>
      {('<p class="hint-text"><strong>Goal:</strong> ' + escape_html(puzzle['objective'].get('description','')) + '</p>') if puzzle.get('objective') else ''}
      {('<p class="hint-text">' + escape_html(puzzle.get('transfer_explanation','')) + '</p>') if puzzle.get('transfer_explanation') else ''}
      {('<p class="hint-text">Adapted from <a href="' + escape_html(puzzle['source'].get('url','')) + '" target="_blank" rel="noopener">' + escape_html(puzzle['source'].get('title','external example')) + '</a>. ' + escape_html(puzzle.get('transformation','')) + '</p>') if puzzle.get('source') else ''}
      <div class="feedback" id="pfeedback-{i}"></div>
      <div class="hint-box" id="phint-{i}" style="display:none">
        <strong>Hint:</strong> {escape_html(puzzle['hint'])}
      </div>
    </div>
  </div>
</section>'''
        puzzle_sections.append(section)

    # Build concepts & resources section
    concepts_html = ''
    for concept in lesson_data.get('concepts_learned', []):
        resources_html = ''
        for res in concept.get('resources', []):
            resources_html += f'<li><a href="{escape_html(res["url"])}" target="_blank" rel="noopener">{escape_html(res["title"])}</a></li>'

        anecdote_html = ''
        if concept.get('anecdote'):
            anecdote_html = f'''<div class="anecdote-box">
  <span class="anecdote-label">&#127881; Go Anecdote</span>
  {escape_html(concept['anecdote'])}
</div>'''

        japanese_html = ''
        if concept.get('japanese'):
            japanese_html = f'<div class="concept-japanese">{escape_html(concept["japanese"])}</div>'

        concepts_html += f'''
<div class="concept-card">
  <h3>{escape_html(concept['term'])}</h3>
  {japanese_html}
  <p>{escape_html(concept['description'])}</p>
  {f'<ul class="concept-resources">{resources_html}</ul>' if resources_html else ''}
  {anecdote_html}
</div>'''

    concepts_section = ''
    if concepts_html:
        concepts_section = f'''
<section class="concepts">
  <h2>Go Concepts & Resources</h2>
  <p style="margin-bottom:16px;color:#666;">Here are the formal Go concepts covered in this lesson, along with Japanese terminology and resources for further study.</p>
  {concepts_html}
</section>'''

    # Build JS data
    lessons_js = json.dumps([
        {
            'move_number': l['move_number'],
            'played_move': l['played_move'],
            'preferred_move': l['preferred_move'],
            'player_color': color_for(l),
            'board_size': gi.get('board_size', 9),
            'explanation': l['explanation'],
            'variation_explanation': l['variation_explanation'],
            'point_loss': l.get('point_loss'),
            'played_quality': l.get('played_quality'),
            'refutation': l.get('refutation', []),
            'refutation_explanation': l.get('refutation_explanation', ''),
            'better_line': l.get('better_line', []),
            'evidence': l.get('_evidence', {}),
            'difficulty': l.get('_difficulty', {}),
            'panels': l.get('panels', {}),
            'sequence_explanations': l.get('sequence_explanations', {}),
            'alternative_explanations': l.get('alternative_explanations', {}),
            'alternatives': [
                {
                    'move': a['move'],
                    'loss_vs_best': a.get('loss_vs_best'),
                    'quality': a.get('quality'),
                    'explanation': a.get('explanation', ''),
                }
                for a in l.get('alternatives', [])
            ],
        }
        for l in lesson_data.get('lessons', [])
    ])

    puzzles_js = json.dumps(lesson_data.get('puzzles', []))
    good_moves_js = json.dumps(good_moves_js_list)
    moves_js = json.dumps(moves)
    lessons_js = lessons_js.replace("<", "\\u003c")
    puzzles_js = puzzles_js.replace("<", "\\u003c")
    good_moves_js = good_moves_js.replace("<", "\\u003c")
    moves_js = moves_js.replace("<", "\\u003c")

    # Overall feedback
    overview_html = format_paragraphs(lesson_data.get('overall_feedback', ''))

    # Game arc (phase-by-phase narrative) — skipped if absent
    arc_section = build_arc_section(lesson_data.get('game_arc'))
    progress_section = build_progress_section(lesson_data.get('progress'))

    # Game title
    title = escape_html(lesson_data.get('game_title', 'Go Game Review'))
    black = escape_html(lesson_data.get('players', {}).get('black', 'Black'))
    white = escape_html(lesson_data.get('players', {}).get('white', 'White'))
    result = escape_html(lesson_data.get('result', ''))

    html = f'''<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
{CSS}
.perspective-bar {{ position:sticky;top:0;z-index:10;display:flex;justify-content:space-between;gap:16px;align-items:center;padding:18px 24px;background:#f6f8f2;border:1px solid #d8dfcf;box-shadow:0 4px 15px #18302512; }}
.perspective-bar p {{ margin:3px 0 0;color:#617062;font-size:13px; }}
.perspective-options {{ display:flex;gap:5px; }}
.perspective-options button,.sequence-controls button {{ border:1px solid #cbd6c8;border-radius:8px;background:white;padding:9px 13px;color:#294436;cursor:pointer; }}
.perspective-options button.active {{ background:#294c3d;color:white;border-color:#294c3d; }}
.evidence-panels {{ display:grid;grid-template-columns:1fr 1fr;gap:8px;margin:16px 0; }}
.evidence-panels button {{ text-align:left;border:1px solid #dce2d7;border-radius:10px;background:#fafbf7;padding:13px;cursor:pointer;color:#294436; }}
.evidence-panels button span {{ display:block;font-size:12px;margin-top:5px;color:#657367; }}
.evidence-panels button.active {{ background:#e7f0e2;border-color:#527352;box-shadow:inset 3px 0 #527352; }}
.sequence-controls {{ display:flex;justify-content:center;gap:7px;margin:12px 0 6px; }}
.sequence-controls button:disabled {{ opacity:.4;cursor:default; }}
.sequence-range {{ width:100%;accent-color:#41694e; }}
.sequence-state {{ text-align:center;min-height:20px;color:#617062;font-size:13px; }}
.branch-choice {{ width:100%;padding:9px;margin-bottom:12px;border:1px solid #cbd6c8;border-radius:8px;background:white; }}
.evidence-facts {{ color:#657168;font-size:12px;line-height:1.6;margin:12px 0; }}
.evidence-facts p {{ margin:5px 0; }}
button:focus-visible,select:focus-visible {{ outline:3px solid #ba8c36;outline-offset:3px; }}
@media(max-width:700px) {{ .perspective-bar {{ position:static;flex-direction:column;align-items:stretch; }} .perspective-options button {{ flex:1; }} }}
</style>
</head>
<body>
<header>
  <h1>{title}</h1>
  <div class="game-info">
    <span>Black: <strong>{black}</strong></span>
    <span>White: <strong>{white}</strong></span>
    <span>Result: <strong>{result}</strong></span>
  </div>
</header>

<details class="game-context">
<summary style="cursor:pointer;padding:18px;font-weight:700">Game overview, development and progress</summary>
<section class="overview">
  <h2>Game Overview</h2>
  <div class="overview-text">
    {overview_html}
  </div>
</section>

{arc_section}

{progress_section}
</details>

<section class="perspective-bar" aria-label="Lesson perspective">
  <div><strong>Explore the sequences</strong><p>Choose whose choices you want to follow.</p></div>
  <div class="perspective-options">
    <button data-perspective="engine" aria-pressed="true" class="active" onclick="setPerspective('engine')">Engine</button>
    <button data-perspective="student" aria-pressed="false" onclick="setPerspective('student')">Your level{(' · ' + escape_html(str(parsed_data.get('profiles',{}).get('student') or '').replace('rank_',''))) if parsed_data.get('profiles',{}).get('student') else ''}</button>
    <button data-perspective="target" aria-pressed="false" onclick="setPerspective('target')">Target level{(' · ' + escape_html(str(parsed_data.get('profiles',{}).get('target') or '').replace('rank_',''))) if parsed_data.get('profiles',{}).get('target') else ''}</button>
  </div>
</section>

{''.join(lesson_sections)}

{good_moves_section}

{''.join(puzzle_sections)}

{concepts_section}

<footer>
  <p>Generated from KataGo analysis · Interactive Go lesson</p>
</footer>

<script>
{GO_BOARD_JS}

const gameSetup = {json.dumps({"black":gi.get("setup_black",[]),"white":gi.get("setup_white",[])})};
const allMoves = {moves_js};
const lessonData = {lessons_js};
const puzzleData = {puzzles_js};
const goodMoveData = {good_moves_js};

window.onload = function() {{
  for (let i = 0; i < lessonData.length; i++) {{
    initLesson(i, lessonData[i], allMoves);
  }}
  for (let i = 0; i < goodMoveData.length; i++) {{
    initGoodMove(i, goodMoveData[i], allMoves);
  }}
  for (let i = 0; i < puzzleData.length; i++) {{
    initPuzzle(i, puzzleData[i]);
  }}
}};
</script>
</body>
</html>'''

    return html


def main():
    if len(sys.argv) != 4:
        print(f"Usage: {sys.argv[0]} <parsed.json> <lesson.json> <output.html>")
        sys.exit(1)

    with open(sys.argv[1], 'r') as f:
        parsed_data = json.load(f)
    with open(sys.argv[2], 'r') as f:
        lesson_data = json.load(f)

    from validate_lesson import validate
    errors, warnings = validate(parsed_data, lesson_data)
    if errors:
        raise SystemExit("Lesson validation failed:\n" + "\n".join(errors))
    html = generate_html(parsed_data, lesson_data)

    with open(sys.argv[3], 'w') as f:
        f.write(html)

    print(f"Generated {sys.argv[3]} ({len(html)} bytes)", file=sys.stderr)


if __name__ == '__main__':
    main()
