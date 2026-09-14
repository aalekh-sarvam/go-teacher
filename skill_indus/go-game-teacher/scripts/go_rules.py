"""Small replay validator for evidence and quizzes; not a Go solver.

Handles setup, captures, pass, suicide, simple ko and common superko rules. Keeping move
colours explicit avoids assuming that White is always the second move in a record.
"""
import copy
import re
COLS = 'ABCDEFGHJKLMNOPQRSTUVWXYZ'

class Position:
    def __init__(self, size=19, size_y=None, black=(), white=(), to_move='B', rules='japanese'):
        self.sx, self.sy = size, size_y or size
        if not (2 <= self.sx <= 25 and 2 <= self.sy <= 25):
            raise ValueError('unsupported board size')
        self.board = {}
        self.to_move, self.rules = to_move, rules.lower()
        for color, points in [('B', black), ('W', white)]:
            for move in points:
                point = self.xy(move)
                if point is None or point in self.board:
                    raise ValueError(f'invalid or duplicate setup {move}')
                self.board[point] = color
        self.history = [(self.key(), to_move)]

    def xy(self, move):
        if move == 'pass': return None
        if not isinstance(move, str) or not re.fullmatch(r'[A-Z][1-9][0-9]*', move):
            raise ValueError(f'invalid coordinate {move}')
        if move[0] not in COLS[:self.sx]: raise ValueError(f'column outside board: {move}')
        row = int(move[1:])
        if not 1 <= row <= self.sy: raise ValueError(f'row outside board: {move}')
        return COLS.index(move[0]), self.sy - row

    def key(self): return tuple(sorted(self.board.items()))
    def clone(self): return copy.deepcopy(self)
    def neighbours(self, p):
        x, y = p
        for dx, dy in [(1,0),(-1,0),(0,1),(0,-1)]:
            if 0 <= x+dx < self.sx and 0 <= y+dy < self.sy: yield x+dx, y+dy
    def group(self, p):
        color = self.board[p]; stones = {p}; todo = [p]; liberties = set()
        while todo:
            for n in self.neighbours(todo.pop()):
                if n not in self.board: liberties.add(n)
                elif self.board[n] == color and n not in stones:
                    stones.add(n); todo.append(n)
        return stones, liberties

    def play(self, move, color=None):
        color = color or self.to_move
        if color not in ('B', 'W') or color != self.to_move: raise ValueError('wrong player to move')
        next_player = 'W' if color == 'B' else 'B'
        p = self.xy(move)
        if p is None:
            self.to_move = next_player; self.history.append((self.key(), next_player)); return
        if p in self.board: raise ValueError(f'occupied: {move}')
        old = self.board.copy(); self.board[p] = color
        for n in list(self.neighbours(p)):
            if n in self.board and self.board[n] != color:
                stones, libs = self.group(n)
                if not libs:
                    for stone in stones: del self.board[stone]
        stones, libs = self.group(p)
        if not libs:
            if self.rules not in ('new-zealand','tromp-taylor'):
                self.board = old; raise ValueError(f'suicide: {move}')
            for stone in stones: del self.board[stone]
        key = self.key()
        if self.rules in ('japanese', 'korean'):
            repeat = len(self.history) >= 2 and self.history[-2][0] == key
        elif self.rules in ('aga','bga','chinese-kgs'):
            repeat = (key, next_player) in self.history
        else:
            repeat = any(k == key for k, _ in self.history)
        if repeat:
            self.board = old; raise ValueError(f'ko repetition: {move}')
        self.to_move = next_player; self.history.append((key, next_player))

    @classmethod
    def from_parsed(cls, parsed, turn):
        gi = parsed['game_info']
        p = cls(gi.get('board_size',19),gi.get('board_size_y'),gi.get('setup_black',[]),gi.get('setup_white',[]),gi.get('initial_player',parsed['moves'][0]['color'] if parsed['moves'] else 'B'),gi.get('rules','japanese'))
        if not isinstance(turn,int) or not 0 <= turn <= len(parsed['moves']): raise ValueError('invalid sequence origin')
        for m in parsed['moves'][:turn]:
            p.to_move = m['color']
            p.play(m['move'])
        if turn < len(parsed['moves']): p.to_move = parsed['moves'][turn]['color']
        return p


def sequence_position(parsed, sequence):
    p = Position.from_parsed(parsed, sequence['from_turn'])
    first = {'Black':'B','White':'W'}.get(sequence['first_to_move'], sequence['first_to_move'])
    if p.to_move != first: raise ValueError('sequence first player differs from game origin')
    for mv in sequence['moves']: p.play(mv)
    return p
