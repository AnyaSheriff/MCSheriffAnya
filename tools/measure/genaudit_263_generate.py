"""Generate black-box census windows through each server's own console.

No archives are inspected. Run after the release build. The official server
must already be locally available; its JAR is only passed to java -jar.
All outputs go outside the repository. Resume skips *saved* complete batches.
"""
import argparse
import json
import math
from pathlib import Path
import queue
import re
import shutil
import subprocess
import struct
import threading
import time
import zlib

from genaudit_263_io import Reader, level, region_dir, registry
import census_263 as census


class Console:
    def __init__(self, args, cwd, ready, timeout=300):
        self.log = (cwd / 'server.out').open('a')
        self.process = subprocess.Popen(args, cwd=cwd, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                        text=True, bufsize=1)
        (cwd / 'server.pid').write_text(str(self.process.pid))
        self.lines = queue.Queue()
        self.serial = 0
        def drain():
            for line in self.process.stdout:
                self.log.write(line)
                self.log.flush()
                self.lines.put(line)
        self.thread = threading.Thread(target=drain, daemon=True)
        self.thread.start()
        try:
            self.wait(ready, timeout)
        except BaseException:
            self.process.terminate()
            self.process.wait(timeout=60)
            raise

    def wait(self, expression, timeout):
        end = time.monotonic() + timeout
        received = []
        while time.monotonic() < end:
            if self.process.poll() is not None:
                raise RuntimeError(f'Server exited {self.process.returncode}; see server.out')
            try:
                line = self.lines.get(timeout=1)
            except queue.Empty:
                continue
            received.append(line)
            if re.search(expression, line):
                return ''.join(received)
        raise TimeoutError(f'Server did not reach {expression}; see server.out')

    def command(self, command, timeout=1800):
        self.serial += 1
        marker = f'GENAUDIT_SYNC_{self.serial}_DONE'
        self.process.stdin.write(command + '\nsay ' + marker + '\n')
        self.process.stdin.flush()
        return self.wait(marker, timeout)

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.write('stop\n')
            self.process.stdin.flush()
            try:
                self.process.wait(timeout=180)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                self.process.wait(timeout=60)
        self.thread.join(timeout=5)
        self.log.close()


def windows(spawn, radius):
    x, z = spawn
    # Include both ends, also when spawn is not chunk-aligned.
    ow = [math.floor((x-radius)/16), math.floor((z-radius)/16),
          math.floor((x+radius)/16), math.floor((z+radius)/16)]
    return {'overworld': [ow], 'the_nether': [[-16, -16, 16, 16]],
            'the_end': [[-12, -12, 12, 12], [112, 112, 144, 144], [-144, 112, -112, 144]]}


def saved(world, dim, bounds, kind):
    x0, z0, x1, z1 = bounds
    folder = region_dir(world, dim)
    wanted = {(x, z) for x in range(x0, x1+1) for z in range(z0, z1+1)}
    result = set()
    # Reader validates CRC, coordinate and full status, not just header allocation.
    reader = Reader()
    suffix = 'mca' if kind == 'vanilla' else 'mcs'
    for rx in range(x0//32, x1//32+1):
        for rz in range(z0//32, z1//32+1):
            file = folder / f'r.{rx}.{rz}.{suffix}'
            if file.exists():
                if kind == 'vanilla':
                    result.update((r['xPos'],r['zPos']) for r in census.chunks(file)
                                  if (r['xPos'],r['zPos']) in wanted)
                else:
                    data=file.read_bytes()
                    if data[:5]!=b'MCS\x01\x01':
                        raise ValueError(f'Bad MCS header: {file}')
                    for cx,cz in wanted:
                        if cx//32!=rx or cz//32!=rz:
                            continue
                        off,size=struct.unpack_from('>II',data,8+((cx%32)+(cz%32)*32)*8)
                        if not size:
                            continue
                        record=data[off*256:off*256+size]
                        if len(record)!=size:
                            continue
                        crc,n=struct.unpack_from('>II',record)
                        if n==len(record)-8 and zlib.crc32(record[8:])==crc:
                            result.add((cx,cz))
    return len(result)


def generate(args):
    base = Path(args.output).resolve()
    base.mkdir(parents=True, exist_ok=True)
    if base.is_relative_to(Path(__file__).resolve().parents[2]):
        raise ValueError('Generated worlds must be outside the repository')
    owner = level(args.owner_world)
    if args.owner_seed is not None:owner['seed']=args.owner_seed
    seeds = [('owner', owner['seed']), ('second', args.second_seed)]
    for label, seed in seeds:
        if args.label and args.label != label:
            continue
        plan_file = base / f'{label}-targets-plan.json' if args.targets else base / f'{label}-plan.json'
        plan = json.loads(plan_file.read_text()) if plan_file.exists() else None
        if plan is not None:
            # A live owner's world may be reset during a long collection.
            # Existing paired measurements always retain their recorded seed.
            if label=='owner' and args.owner_seed is not None and plan['seed']!=args.owner_seed:
                raise ValueError('Explicit owner seed differs from saved plan; choose a new output directory')
            seed=plan['seed']
        for kind in ('vanilla', 'ours'):
            if args.only and args.only != kind:
                continue
            cwd = base / f'{label}-{kind}'
            cwd.mkdir(exist_ok=True)
            world = cwd / 'world'
            common = (f'level-seed={seed}\nlevel-type=minecraft:normal\nonline-mode=false\n'
                      'view-distance=3\nsimulation-distance=3\npause-when-empty-seconds=0\n'
                      'difficulty=peaceful\n')
            if kind == 'vanilla':
                (cwd / 'eula.txt').write_text('eula=true\n')
                (cwd / 'server.properties').write_text(common +
                    'server-ip=127.0.0.1\nserver-port=26180\nenable-rcon=false\n'
                    'enable-query=false\nmax-tick-time=-1\nspawn-protection=0\n')
                command = [args.java, '-Xms512M', '-Xmx3G', '-XX:ActiveProcessorCount=4',
                           '-jar', str(Path(args.jar).resolve()), 'nogui']
                ready = r'Done \('
            else:
                (cwd / 'config').mkdir(exist_ok=True)
                (cwd / 'config/server.properties').write_text(common + 'server-port=26280\n')
                (cwd / 'config/mcsa.properties').write_text(
                    'java-version=26.3\nsend-error-reports=false\nauth-enabled=false\n'
                    'offline-license-mode=disabled\nbedrock-port=26281\nviaproxy=false\n'
                    'autumn-forests=false\npink-cherry-groves=false\nconsole-colors=never\n')
                command = [str(Path(args.ours).resolve())]
                ready = r'Сервер запущен на'
            print(f'Start {label} {kind} seed={seed}', flush=True)
            server = Console(command, cwd, ready)
            try:
                # Startup bind precedes world loading in ours: marker waits for console readiness.
                server.command('gamerule randomTickSpeed 0')
                server.command('gamerule minecraft:random_tick_speed 0')
                server.command('gamerule doMobSpawning false')
                server.command('gamerule minecraft:spawn_mobs false')
                seed_reply = server.command('seed')
                if str(seed) not in seed_reply:
                    raise ValueError('Server /seed did not confirm the requested seed')
                (cwd / 'observed-seed.json').write_text(json.dumps({'seed':seed,'console_reply':seed_reply})+'\n')
                server.command('save-all flush')
                if plan is None:
                    center = owner['spawn'] if label == 'owner' else level(world)['spawn']
                    plan = {'label': label, 'seed': seed, 'center': center, 'radius_blocks': args.radius,
                            'windows': windows(center, args.radius),
                            'settings': 'peaceful; random ticks zero; no players; own visual extensions disabled'}
                    if args.targets:
                        if kind != 'vanilla':
                            raise ValueError('Discover targets on the official server first')
                        _,biomes,_=registry()
                        groups={'overworld':biomes[:54]+biomes[65:], 'the_nether':biomes[55:60], 'the_end':biomes[60:65]}
                        if args.camps_only:groups={d:[] for d in groups}
                        targets=[]
                        plan['windows']={d:[] for d in groups}
                        for dim,names in groups.items():
                            for name in names:
                                reply=server.command(f'execute in minecraft:{dim} run locate biome minecraft:{name}')
                                found=re.search(r'\[(-?\d+), (?:~|-?\d+), (-?\d+)\]',reply)
                                entry={'biome':name,'dimension':dim,'console_reply':reply,'bounds':None}
                                if found:
                                    x,z=map(int,found.groups());cx,cz=x//16,z//16
                                    r=16 if 'ocean' in name else 8 if name in ('eroded_badlands','ice_spikes') else 4
                                    entry.update({'position':[x,z],'bounds':[cx-r,cz-r,cx+r,cz+r]})
                                    plan['windows'][dim].append(entry['bounds'])
                                targets.append(entry)
                                print(label,'locate',dim,name,entry.get('position','not found'),flush=True)
                        plan['biome_targets']=targets
                        structure_names={
                            'overworld':['village_plains','village_desert','village_savanna','village_snowy','village_taiga',
                                         'pillager_outpost','desert_pyramid','jungle_pyramid','swamp_hut','igloo',
                                         'shipwreck','shipwreck_beached','ocean_ruin_cold','ocean_ruin_warm',
                                         'monument','mansion','ancient_city','trail_ruins','trial_chambers',
                                         'mineshaft','mineshaft_mesa','stronghold','buried_treasure','ruined_portal','abandoned_camp'],
                            'the_nether':['fortress','bastion_remnant','nether_fossil','ruined_portal_nether'],
                            'the_end':['end_city']}
                        camp_biomes=['bamboo_jungle','sparse_jungle','forest','flower_forest','birch_forest',
                                     'old_growth_birch_forest','pale_garden','taiga','snowy_taiga','old_growth_pine_taiga',
                                     'old_growth_spruce_taiga','meadow','cherry_grove','savanna','windswept_forest',
                                     'swamp','wooded_badlands','dappled_forest']
                        camp_names=['abandoned_camp_'+b for b in camp_biomes]
                        if args.camps_only:
                            structure_names={'overworld':camp_names+['mineshaft_mesa'],'the_nether':[],'the_end':[]}
                        else:
                            structure_names['overworld'].remove('abandoned_camp')
                            structure_names['overworld']+=camp_names
                        stargets=[]
                        for dim,names in structure_names.items():
                            for name in names:
                                reply=server.command(f'execute in minecraft:{dim} run locate structure minecraft:{name}')
                                found=re.search(r'\[(-?\d+), (?:~|-?\d+), (-?\d+)\]',reply)
                                if not found and (name.startswith('abandoned_camp_') or name=='mineshaft_mesa'):
                                    biome=name.removeprefix('abandoned_camp_') if name!='mineshaft_mesa' else 'badlands'
                                    near=server.command(f'execute in minecraft:{dim} run locate biome minecraft:{biome}')
                                    location=re.search(r'\[(-?\d+), (?:~|-?\d+), (-?\d+)\]',near)
                                    if location:
                                        px,pz=location.groups()
                                        positioned=server.command(f'execute in minecraft:{dim} positioned {px} 64 {pz} run locate structure minecraft:{name}')
                                        found=re.search(r'\[(-?\d+), (?:~|-?\d+), (-?\d+)\]',positioned)
                                        reply+=near+positioned
                                entry={'structure':name,'dimension':dim,'console_reply':reply,'bounds':None}
                                if found:
                                    x,z=map(int,found.groups());cx,cz=x//16,z//16
                                    r=12 if name=='fortress' else 8
                                    entry.update({'position':[x,z],'bounds':[cx-r,cz-r,cx+r,cz+r]})
                                    plan['windows'][dim].append(entry['bounds'])
                                stargets.append(entry)
                                print(label,'locate structure',dim,name,entry.get('position','not found'),flush=True)
                        plan['structure_targets']=stargets
                        plan['sample_kind']='Targeted biome windows; do not combine these with unbiased spawn-area distributions'
                    plan_file.write_text(json.dumps(plan, indent=2) + '\n')
                for dim, areas in plan['windows'].items():
                    if not areas:continue
                    sample_world, sample_dim = world, dim
                    if kind == 'ours' and dim != 'overworld':
                        # Our console has no execute-in/forceload. Run the same
                        # release server with its supported Nether/End world kind,
                        # then move the saved regions into the aggregate world.
                        # No terrain or structure blocks are placed by commands.
                        server.close()
                        dimcwd = base / f'{label}-ours-{dim}'
                        (dimcwd / 'config').mkdir(parents=True,exist_ok=True)
                        (dimcwd / 'config/server.properties').write_text(common +
                            f'server-port=26280\nlevel-type=minecraft:{dim}\n')
                        shutil.copyfile(cwd / 'config/mcsa.properties',dimcwd / 'config/mcsa.properties')
                        sample_world, sample_dim = dimcwd / 'world', 'overworld'
                        target = world / 'dimensions/minecraft' / dim / 'region'
                        # An empty pre-created root is not a recognized world:
                        # let the release server initialize its own profile.
                        if sample_world.exists() and not (sample_world/'level.dat').exists():
                            empty_region=sample_world/'region'
                            if empty_region.exists() and not list(empty_region.iterdir()):
                                empty_region.rmdir()
                            if not list(sample_world.iterdir()):
                                sample_world.rmdir()
                        # Resume previously moved measurements without copying.
                        if target.exists() and list(target.iterdir()) and not (sample_world/'region').exists():
                            sample_world.mkdir(exist_ok=True)
                            if not (sample_world/'level.dat').exists():
                                shutil.copyfile(world/'mcsa-world.json',sample_world/'mcsa-world.json')
                                shutil.copyfile(target.parent/'level.dat',sample_world/'level.dat')
                            target.rename(sample_world/'region')
                        server = Console(command,dimcwd,ready)
                        server.command('gamerule random_tick_speed 0')
                        server.command('gamerule spawn_mobs false')
                    prefix = f'execute in minecraft:{dim} run ' if kind=='vanilla' else ''
                    server.command(prefix + 'forceload remove all')
                    for x0, z0, x1, z1 in areas:
                        for x in range(x0, x1+1, 16):
                            for z in range(z0, z1+1, 16):
                                bounds = [x, z, min(x+15, x1), min(z+15, z1)]
                                total = (bounds[2]-x+1) * (bounds[3]-z+1)
                                if saved(sample_world, sample_dim, bounds, kind) == total:
                                    print(label, kind, dim, bounds, 'already saved', flush=True)
                                    continue
                                t = time.monotonic()
                                reply = server.command(prefix + f'forceload add {x*16} {z*16} {bounds[2]*16+15} {bounds[3]*16+15}')
                                if any(s in reply for s in ('Unknown','Неизвест','Incorrect','Неверн')):
                                    raise RuntimeError(reply)
                                deadline = time.monotonic() + 1800
                                while True:
                                    time.sleep(2)
                                    server.command('save-all flush')
                                    count = saved(sample_world, sample_dim, bounds, kind)
                                    if count == total:
                                        break
                                    if time.monotonic() > deadline:
                                        raise TimeoutError(f'{kind}/{dim}/{bounds}: saved {count}/{total}')
                                server.command(prefix + 'forceload remove all')
                                print(label, kind, dim, bounds, f'{total} chunks {time.monotonic()-t:.1f}s', flush=True)
                    if kind == 'ours' and dim != 'overworld':
                        server.command('save-all flush')
                        server.close()
                        target.parent.mkdir(parents=True,exist_ok=True)
                        if target.exists():
                            if list(target.iterdir()):
                                raise ValueError(f'Refusing to replace nonempty region directory {target}')
                            target.rmdir()
                        (sample_world/'region').rename(target)
            finally:
                server.close()
            completion='generation-targets-complete.json' if args.targets else 'generation-complete.json'
            (cwd / completion).write_text(json.dumps(plan, indent=2) + '\n')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', required=True)
    p.add_argument('--jar', required=True)
    p.add_argument('--ours', required=True)
    p.add_argument('--java', default='/usr/lib/jvm/java-25-openjdk/bin/java')
    p.add_argument('--owner-world', default='/home/lenox/.cache/mcsheriffanya/test-263/world')
    p.add_argument('--owner-seed',type=int,help='Freeze the captured owner seed for new supplementary datasets')
    p.add_argument('--second-seed', type=int, default=100554032945340)
    p.add_argument('--radius', type=int, default=1536)
    p.add_argument('--only', choices=['vanilla', 'ours'])
    p.add_argument('--label', choices=['owner', 'second'])
    p.add_argument('--targets',action='store_true',help='Discover targeted biome/structure windows on vanilla, then generate the same windows on ours')
    p.add_argument('--camps-only',action='store_true',help='Discover the eighteen biome-specific camp IDs in a separate target dataset')
    a=p.parse_args()
    if a.camps_only:a.targets=True
    generate(a)
