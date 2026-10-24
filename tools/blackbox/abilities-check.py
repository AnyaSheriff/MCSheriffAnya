#!/usr/bin/env python3
"""Чёрный ящик: вики Player.dat_format, «abilities», «flying», «mayfly»."""
import argparse
import json
import os
import sys
import select
import socket
import struct
import time
import uuid
import zlib


def varint(n):
    out = bytearray()
    while n > 127:
        out.append((n & 127) | 128)
        n >>= 7
    return bytes(out + bytes([n]))


def read_varint(data, at=0):
    n = 0
    for shift in range(0, 35, 7):
        b = data[at]
        at += 1
        n |= (b & 127) << shift
        if b < 128:
            return n, at
    raise ValueError('varint')


def string(text):
    raw = text.encode()
    return varint(len(raw)) + raw


class Client:
    def __init__(self, port):
        self.sock = socket.create_connection(('127.0.0.1', port), 10)
        self.sock.settimeout(15)
        self.compression = None
        self.state = 'login'
        self.abilities = []
        self.chats = 0
        self.send(0, varint(775) + string('localhost') + struct.pack('>H', port) + varint(2))
        self.send(0, string('blackbox') + uuid.UUID('26d479c9-4844-3b0a-ba59-8cc656622a78').bytes)
        deadline = time.monotonic() + 30
        while not self.abilities:
            if time.monotonic() > deadline:
                raise TimeoutError('join abilities')
            self.receive()
        self.pump(1)

    def exact(self, size):
        out = bytearray()
        while len(out) < size:
            chunk = self.sock.recv(size - len(out))
            if not chunk:
                raise ConnectionError('server closed connection')
            out.extend(chunk)
        return bytes(out)

    def send(self, packet, body=b''):
        data = varint(packet) + body
        if self.compression is not None:
            data = varint(len(data)) + zlib.compress(data) if len(data) >= self.compression else b'\0' + data
        self.sock.sendall(varint(len(data)) + data)

    def receive(self):
        prefix = bytearray()
        while True:
            prefix.extend(self.exact(1))
            if prefix[-1] < 128:
                break
        size, _ = read_varint(prefix)
        data = self.exact(size)
        if self.compression is not None:
            length, at = read_varint(data)
            data = zlib.decompress(data[at:]) if length else data[at:]
        packet, at = read_varint(data)
        body = data[at:]
        if os.environ.get("ABILITIES_TRACE"):
            print(self.state, hex(packet), len(body), file=sys.stderr, flush=True)
        if self.state == 'login':
            if packet == 3:
                self.compression, _ = read_varint(body)
            elif packet == 2:
                self.send(3)
                self.state = 'configuration'
                self.send(0, string('en_us') + bytes([2]) + varint(0) + bytes([1, 127]) + varint(1) + bytes([0, 1]) + varint(0))
            elif packet == 0:
                raise RuntimeError(f'login disconnect {body!r}')
        elif self.state == 'configuration':
            if packet == 14:
                self.send(7, b'\0')
            elif packet == 3:
                self.send(3)
                self.state = 'play'
            elif packet in (4, 5):
                self.send(packet, body)
            elif packet == 2:
                raise RuntimeError(f'configuration disconnect {body!r}')
        else:
            if packet == 0x40:
                flags, fly, walk = struct.unpack('>Bff', body)
                self.abilities.append([flags, fly, walk])
            elif packet == 0x48:
                teleport, _ = read_varint(body)
                self.send(0, varint(teleport))
                self.send(0x2c)
            elif packet == 0x2c:
                self.send(0x1c, body)
            elif packet == 0x0b:
                self.send(0x0b, struct.pack('>f', 8.0))
            elif packet == 0x79:
                self.chats += 1
            elif packet == 0x20:
                raise RuntimeError(f'play disconnect {body!r}')

    def pump(self, seconds=0.5):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if select.select([self.sock], [], [], max(0, end - time.monotonic()))[0]:
                self.receive()

    def command(self, text):
        before = self.chats
        self.send(7, string(text))
        deadline = time.monotonic() + 30
        while self.chats == before:
            if time.monotonic() > deadline:
                raise TimeoutError(f'command {text}')
            self.pump(0.1)
        self.pump(0.25)

    def close(self):
        self.sock.close()
        time.sleep(0.5)


def scenario(port):
    result = {}
    c = Client(port)
    def snap(name, action=None):
        c.abilities.clear()
        if action:
            action()
        c.pump()
        result[name] = c.abilities[:]
    if c.abilities[-1][0] != 7:
        c.command('gamemode spectator blackbox')
    c.command('gamemode creative blackbox')
    c.command('execute in minecraft:overworld run tp blackbox 0 100 0')
    c.command('gamemode survival blackbox')
    snap('enter_creative', lambda: c.command('gamemode creative blackbox'))
    c.send(0x28, b'\x02')
    c.pump()
    c.close()
    c = Client(port)
    result['flying_rejoin'] = c.abilities[:]
    snap('unchanged_creative', lambda: (c.send(7, string('gamemode creative blackbox')), c.pump(1)))
    snap('survival_stops_flight', lambda: c.command('gamemode survival blackbox'))
    c.send(0x28, b'\xff')
    c.pump()
    snap('creative_after_denied_flight', lambda: c.command('gamemode creative blackbox'))
    c.close()
    c = Client(port)
    result['grounded_rejoin'] = c.abilities[:]
    c.send(0x28, b'\x02')
    c.pump()
    snap('spectator', lambda: c.command('gamemode spectator blackbox'))
    c.send(0x28, b'\0')
    c.pump()
    c.close()
    c = Client(port)
    result['spectator_stopped_rejoin'] = c.abilities[:]
    snap('spectator_to_creative', lambda: c.command('gamemode creative blackbox'))
    c.send(0x28, b'\x02')
    c.pump()
    snap('dimension_flying', lambda: c.command('execute in minecraft:the_nether run tp blackbox 0 100 0'))
    snap('adventure', lambda: c.command('gamemode adventure blackbox'))
    snap('creative_from_adventure', lambda: c.command('gamemode creative blackbox'))
    c.send(0x28, b'\x02')
    c.pump()
    c.command('execute in minecraft:overworld run tp blackbox 0 100 0')
    c.send(0x28, b'\x02')
    c.pump()
    c.command('kill blackbox')
    snap('death_respawn', lambda: (c.send(0x0c, b'\0'), c.pump(3)))
    c.close()
    expected = {"enter_creative": [13, 13], "flying_rejoin": [15],
        "unchanged_creative": [], "survival_stops_flight": [0, 0], "creative_after_denied_flight": [13, 13],
        "grounded_rejoin": [13], "spectator": [7, 7], "spectator_stopped_rejoin": [7], "spectator_to_creative": [15, 15],
        "dimension_flying": [15], "adventure": [0, 0], "creative_from_adventure": [13, 13],
        "death_respawn": [13]}
    assert {name: [v[0] for v in values] for name, values in result.items()} == expected, result
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('port', type=int)
    parser.add_argument('--output')
    args = parser.parse_args()
    result = scenario(args.port)
    text = json.dumps(result, indent=2) + '\n'
    if args.output:
        with open(args.output, 'w') as out:
            out.write(text)
    print(text)
