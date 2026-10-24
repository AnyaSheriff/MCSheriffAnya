#!/usr/bin/env python3
"""Независимое чтение книги рецептов по закреплённым JSON и наблюдениям сети."""
import io
import struct

class Decoder:
    def __init__(self, payload, item_names):
        self.data = io.BytesIO(payload)
        self.names = item_names
        self.offsets = []

    def take(self, n):
        result = self.data.read(n)
        if len(result) != n:
            raise ValueError(f'truncated at {self.data.tell()}')
        return result

    def uint(self, maximum=2147483647):
        n = 0
        for i in range(5):
            b = self.take(1)[0]
            n |= (b & 127) << (7*i)
            if b < 128:
                if n > maximum:
                    raise ValueError(f'out of range {n} at {self.data.tell()}')
                return n
        raise ValueError('invalid VarInt')

    def boolean(self):
        return bool(self.uint(1))

    def item(self):
        ident = self.uint()
        if ident not in self.names or self.names[ident] == 'air':
            raise ValueError(f'unknown item {ident} at {self.data.tell()}')
        return self.names[ident]

    def idset(self):
        n = self.uint(65537)
        if n:
            return [self.item() for _ in range(n-1)]
        tag = self.take(self.uint(32767)).decode('utf8')
        if not tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789_./:-' for c in tag):
            raise ValueError(f'invalid identifier {tag!r}')
        return {'tag': tag}

    def slot(self, depth=0):
        if depth > 32:
            raise ValueError('recursive display')
        start = self.data.tell()
        kind = self.uint(10)
        if kind in (0, 1):
            result = [kind]
        elif kind == 4:
            result = [kind, self.item()]
        elif kind == 5:
            result = [kind, self.item(), self.uint(127)]
            added, removed = self.uint(256), self.uint(256)
            for _ in range(added):
                component = self.uint()
                if component == 55:
                    result.append([component, [(self.uint(), self.uint()) for _ in range(self.uint(32))]])
                else:
                    raise ValueError(f'unsupported component {component} at {start}')
            result.extend(self.uint() for _ in range(removed))
        elif kind == 6:
            result = [kind, self.idset()]
        elif kind == 10:
            result = [kind, [self.slot(depth+1) for _ in range(self.uint(4096))]]
        elif kind in (2, 3):
            result = [kind, self.slot(depth+1)]
            if kind == 3:
                result.append(self.uint())
        elif kind == 8:
            result = [kind, self.slot(depth+1), self.slot(depth+1), self.uint()]
            if result[-1] == 0:
                raise ValueError('inline trim pattern')
        elif kind in (7, 9):
            result = [kind, self.slot(depth+1), self.slot(depth+1)]
        else:
            raise ValueError(f'unsupported display {kind} at {start}')
        self.offsets.append((start, self.data.tell(), kind))
        return result

    def recipe(self):
        kind = self.uint(4)
        result = {'kind': kind}
        if kind in (0, 1):
            if kind == 1:
                result['width'], result['height'] = self.uint(3), self.uint(3)
                assert result['width'] and result['height']
            n = self.uint(9)
            if kind == 1:
                assert n == result['width'] * result['height']
            result['ingredients'] = [self.slot() for _ in range(n)]
            result['result'], result['station'] = self.slot(), self.slot()
        else:
            result['slots'] = [self.slot() for _ in range({2:4, 3:3, 4:5}[kind])]
            if kind == 2:
                result['duration'] = self.uint()
                result['xp'] = struct.unpack('>f', self.take(4))[0]
        return result

    def book(self):
        entries = []
        for _ in range(self.uint(65536)):
            start = self.data.tell()
            ident, recipe = self.uint(), self.recipe()
            group, category = self.uint(), self.uint(12)
            requirements = [self.idset() for _ in range(self.uint(1024))] if self.boolean() else None
            flags = self.uint(3)
            entries.append(dict(id=ident, recipe=recipe, group=group, category=category,
                                requirements=requirements, flags=flags, start=start, end=self.data.tell()))
        replace = self.boolean()
        if self.data.read():
            raise ValueError(f'trailing bytes at {self.data.tell()}')
        return entries, replace

if __name__ == '__main__':
    import json
    import sys
    from pathlib import Path
    root = Path(__file__).parent / 'testdata/recipe-book-777'
    names = {int(k): v for k, v in json.loads((root / 'items.json').read_text()).items()}
    if len(sys.argv) > 1:
        payload = Path(sys.argv[1]).read_bytes()
        if sys.argv[1].endswith('ours.bin'):
            payload = payload[1:]
        rows, replace = Decoder(payload, names).book()
        print(json.dumps({'entries': len(rows), 'replace': replace}))
    else:
        cases = json.load(sys.stdin)
        for case in cases:
            rows, replace = Decoder(bytes.fromhex(case['payload']), names).book()
            assert len(rows) == case['entries']
            if case.get('idset_ingredients'):
                for row in rows:
                    recipe = row['recipe']
                    ingredients = recipe.get('ingredients', recipe.get('slots', [])[:1])
                    assert all(slot[0] in (0, 6) for slot in ingredients), row
        print(json.dumps({'packets': len(cases), 'entries': sum(c['entries'] for c in cases)}))
