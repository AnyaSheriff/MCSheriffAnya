# Генерирует сплошной квадрат мира на запущенном сервере оригинала через
# RCON: `forceload` кусками по 16×16 чанков, ждёт, пока чанки лягут в
# region-файлы полными, и снимает загрузку. Сам сервер только запускается
# и слушает команды — его код и jar не читаются.
#
# python3 tools/measure/pregen.py <папка мира> <порт rcon> <пароль> <радиус в чанках> [центр x z в чанках]
import sys, os, socket, struct, time, zlib, glob
sys.path.insert(0, os.path.dirname(__file__))
from plants_census import nbt

class Rcon:
    def __init__(self, port, password):
        self.sock = socket.create_connection(('127.0.0.1', port))
        self.id = 0
        self.call(3, password)

    def call(self, kind, text):
        self.id += 1
        body = text.encode() + b'\0\0'
        self.sock.sendall(struct.pack('<iii', len(body) + 8, self.id, kind) + body)
        size = struct.unpack('<i', self.recv(4))[0]
        data = self.recv(size)
        return data[8:-2].decode('utf8', 'replace')

    def recv(self, n):
        out = b''
        while len(out) < n:
            part = self.sock.recv(n - len(out))
            if not part:
                raise ConnectionError('rcon закрылся')
            out += part
        return out

    def run(self, command):
        return self.call(2, command)

def full_chunks(region_dir, x0, z0, x1, z1):
    """Сколько чанков прямоугольника уже лежит в region-файлах полными."""
    count = 0
    for rx in range(x0 >> 5, (x1 >> 5) + 1):
        for rz in range(z0 >> 5, (z1 >> 5) + 1):
            path = os.path.join(region_dir, 'r.%d.%d.mca' % (rx, rz))
            if not os.path.exists(path):
                continue
            data = open(path, 'rb').read()
            for cx in range(max(x0, rx * 32), min(x1, rx * 32 + 31) + 1):
                for cz in range(max(z0, rz * 32), min(z1, rz * 32 + 31) + 1):
                    i = (cx & 31) + (cz & 31) * 32
                    off = int.from_bytes(data[i*4:i*4+3], 'big')
                    if off == 0 or off * 4096 + 5 > len(data):
                        continue
                    p = off * 4096
                    ln = struct.unpack_from('>i', data, p)[0]
                    try:
                        raw = zlib.decompress(data[p+5:p+4+ln])
                    except zlib.error:
                        continue
                    n = struct.unpack_from('>H', raw, 1)[0]
                    root, _ = nbt(raw, 3 + n, 10)
                    if root.get('Status') in ('minecraft:full', 'full'):
                        count += 1
    return count

def main():
    world, port, password, radius = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4])
    center_x, center_z = (int(sys.argv[5]), int(sys.argv[6])) if len(sys.argv) > 6 else (0, 0)
    region_dir = os.path.join(world, 'dimensions/minecraft/overworld/region')
    rcon = Rcon(port, password)
    areas = [(center_x + x, center_z + z) for x in range(-radius, radius, 16) for z in range(-radius, radius, 16)]
    batch = 4
    started = time.time()
    for at in range(0, len(areas), batch):
        part = areas[at:at + batch]
        for x, z in part:
            print(rcon.run('forceload add %d %d %d %d' % (x * 16, z * 16, x * 16 + 255, z * 16 + 255)), flush=True)
        while True:
            time.sleep(3)
            rcon.run('save-all')
            done = sum(full_chunks(region_dir, x, z, x + 15, z + 15) for x, z in part)
            if done >= 256 * len(part):
                break
        rcon.run('forceload remove all')
        print('готово %d/%d кусков, %.0f с' % (at + len(part), len(areas), time.time() - started), flush=True)
    rcon.run('save-all flush')

if __name__ == '__main__':
    main()
