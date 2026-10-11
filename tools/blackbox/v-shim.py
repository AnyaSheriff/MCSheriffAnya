# UDP-прослойка 19160 → BDS 19150: клиент RakNet 10 → 11 (меняется только номер протокола в Open Connection Request 1).
# В отличие от tmp/raknet-shim.py держит свой исходящий сокет на каждого клиента, так что можно
# подключаться подряд/параллельно.
import socket, select
UP = ('127.0.0.1', 19150)
srv = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
srv.bind(('127.0.0.1', 19160))
by_client = {}
by_sock = {}
while True:
    r, _, _ = select.select([srv] + list(by_sock), [], [])
    for s in r:
        d, a = s.recvfrom(65535)
        if s is srv:
            o = by_client.get(a)
            if o is None:
                o = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                by_client[a] = o
                by_sock[o] = a
            if d[:1] == b'\x05' and len(d) > 17 and d[17] == 10:
                d = d[:17] + b'\x0b' + d[18:]
            o.sendto(d, UP)
        else:
            srv.sendto(d, by_sock[s])
