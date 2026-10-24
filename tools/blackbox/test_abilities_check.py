#!/usr/bin/env python3
"""Регрессии clientbound775/777 по закреплённым ID и отказу оригинала777."""
import importlib.util
import io
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('abilities_check', Path(__file__).with_name('abilities-check.py'))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class Socket:
    def __init__(self, data):
        self.data = io.BytesIO(data)

    def recv(self, size):
        return self.data.read(size)


def receive(version, packet, body):
    client = probe.Client.__new__(probe.Client)
    client.protocol = version
    client.state = 'play'
    client.compression = None
    client.abilities = []
    client.chats = 0
    client.sent = []
    client.send = lambda packet, payload=b'': client.sent.append((packet, payload))
    frame = probe.varint(packet) + body
    client.sock = Socket(probe.varint(len(frame)) + frame)
    client.receive()
    return client


class Packets(unittest.TestCase):
    def test_native_world_border_is_not_legacy_keep_alive(self):
        # Оригинал отказал после ответа40bytes вместо8bytes: ID0x2c777.
        border = struct.pack('>ddddd', 0, 0, 100, 100, 0)
        client = receive(777, 0x2c, border)
        self.assertEqual(client.sent, [])

    def test_other_native_id_collisions_are_ignored(self):
        # protocol.json777: craft_recipe_response, face_player, stop_sound.
        for packet in [0x40, 0x48, 0x79]:
            client = receive(777, packet, b'')
            self.assertEqual(client.sent, [])
            self.assertEqual(client.abilities, [])
            self.assertEqual(client.chats, 0)

    def test_keep_alive_replies_with_exactly_the_received_eight_bytes(self):
        opaque = bytes.fromhex('0123456789abcdef')
        for version, packet in [(775, 0x2c), (777, 0x2d)]:
            self.assertEqual(receive(version, packet, opaque).sent, [(0x1c, opaque)])

    def test_both_abilities_packets_decode_flags_and_speeds(self):
        for version, packet in [(775, 0x40), (777, 0x41)]:
            client = receive(version, packet, struct.pack('>Bff', 15, 0.125, 0.25))
            self.assertEqual(client.abilities, [[15, 0.125, 0.25]])
            self.assertEqual(client.sent, [])

    def test_native_teleport_confirmation_contains_pose(self):
        body = probe.varint(7) + struct.pack('>ddddddffI', 1, 70, 3, 0, 0, 0, 90, 10, 0)
        client = receive(777, 0x49, body)
        self.assertEqual(client.sent, [(0, probe.varint(7) + struct.pack('>dddff', 1, 70, 3, 90, 10)), (0x2c, b'')])


if __name__ == '__main__':
    unittest.main()
