"""Один ограниченный официальный опыт; только новый каталог/свои процессы.

Запускать под глобальным flock. JAR только запускается и хешируется.
Watchdog TERM110s/KILL115s; рабочая часть ограничена100s; весь лимит120s.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import signal
import socket
import subprocess
import threading
import time

import numpy as np

from genaudit_263_io import Reader, region_dir
from genaudit_263_end_geometry import validate_dataset

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n')


def verify_freeze(freeze):
    for path, digest in freeze['input_sha256'].items():
        if sha(REPO/path) != digest:
            raise ValueError('Замороженный вход изменён: '+path)


def free_ports(plan):
    for port, kind in [(plan['tcp_port'],socket.SOCK_STREAM),(plan['udp_port'],socket.SOCK_DGRAM)]:
        with socket.socket(socket.AF_INET,kind) as sock:
            sock.bind(('127.0.0.1',port))


def decode(output, plan, freeze, receipt):
    """Читаем все153forceloadчанка, а не только ответыPlaced."""
    reader = Reader()
    folder = region_dir(output/'official/world','overworld')
    x0,z0,x1,z1=plan['forceload_bounds']
    wanted={(x,z) for x in range(x0//16,x1//16+1) for z in range(z0//16,z1//16+1)}
    all_cells,seen,sources=[],set(),[]
    for rx,rz in sorted({(x//32,z//32) for x,z in wanted}):
        path=folder/f'r.{rx}.{rz}.mca'
        sources.append({'region':path.name,'sha256':sha(path),'bytes':path.stat().st_size})
        for cx,cz,blocks,_,_ in reader.read(path,wanted):
            if (cx,cz) in seen:
                raise ValueError('Повтор чанка')
            seen.add((cx,cz))
            for y,z,x in np.argwhere(blocks==reader.lookup['end_stone']):
                all_cells.append((cx*16+int(x),int(y)-64,cz*16+int(z)))
        if sha(path) != sources[-1]['sha256']:
            raise ValueError('Регион изменился во время чтения')
    if seen != wanted:
        raise ValueError('Неполное покрытие сохранённых чанков')
    radius=plan['read_plot_radius'];assigned=set();observations=[]
    for plot in plan['plots']:
        x,y,z=plot['origin']
        actual=[c for c in all_cells if abs(c[0]-x)<=radius and abs(c[2]-z)<=radius]
        if assigned.intersection(actual):
            raise ValueError('Окна наблюдений перекрылись')
        assigned.update(actual)
        relative=sorted((a-x,b-y,c-z) for a,b,c in actual)
        clipped=any(abs(a)==radius or abs(c)==radius for a,b,c in relative)
        observations.append({'plot_index':plot['plot_index'],'origin':plot['origin'],'cells':relative,
                             'volume':len(relative),'window_boundary_touched':clipped})
    dataset={'schema':1,'observations':observations,'seed':plan['seed'],'tcp_port':plan['tcp_port'],
             'coverage':{'expected':len(wanted),'actual':len(seen),'complete':seen==wanted},
             'stray_endstone_cells':sorted(set(all_cells)-assigned),'sources':sources,
             'freeze_manifest_sha256':sha(output/'freeze.json'),'source_jar_sha256':freeze['source_jar_sha256'],
             'receipt_sha256':sha(output/'receipt.json'),'method':plan['world_kind']}
    write(output/'observations.json',dataset)
    hypothesis=json.loads((REPO/freeze['hypothesis_path']).read_text())
    checked=validate_dataset(dataset,hypothesis)
    checked['geometry_readback_valid']=not dataset['stray_endstone_cells'] and all(o['volume']>0 and not o['window_boundary_touched'] for o in observations)
    checked['passed']=checked['passed'] and checked['geometry_readback_valid']
    checked['observations_sha256']=sha(output/'observations.json')
    write(output/'validation.json',checked)
    return checked


def run(args):
    freeze=json.loads(args.freeze.read_text())
    verify_freeze(freeze)
    if sha(args.jar) != freeze['source_jar_sha256']:
        raise ValueError('Не тот разрешённый JAR')
    plan=json.loads((REPO/freeze['plan_path']).read_text())
    if plan['deadline_seconds'] != 120 or plan['work_cutoff_seconds'] != 100:
        raise ValueError('Неизвестный бюджет опыта')
    free_ports(plan)
    args.output.mkdir(parents=True,exist_ok=False)
    write(args.output/'freeze.json',freeze)
    work=args.output/'official';work.mkdir()
    (work/'eula.txt').write_text('eula=true\n')
    props=(f'server-ip=127.0.0.1\nserver-port={plan["tcp_port"]}\nquery.port={plan["udp_port"]}\nenable-query=true\n'
           f'online-mode=false\nlevel-seed={plan["seed"]}\nlevel-type=minecraft:flat\n'
           'generator-settings={"layers":[],"biome":"minecraft:the_void","structures":{}}\n'
           'view-distance=2\nsimulation-distance=2\nspawn-protection=0\nmax-tick-time=-1\npause-when-empty-seconds=0\n')
    (work/'server.properties').write_text(props)
    argv=[str(args.java),'-Xmx1G','-XX:ActiveProcessorCount=2','-jar',str(args.jar.resolve()),'nogui']
    receipt={'status':'incomplete','seed':plan['seed'],'commands':[],'argv':argv,'ports':[plan['tcp_port'],plan['udp_port']],
             'freeze_manifest_sha256':sha(args.output/'freeze.json'),'source_jar_sha256':freeze['source_jar_sha256'],
             'properties_sha256':sha(work/'server.properties'),'plots':[],'watchdog_actions':[]}
    process=None;finished=threading.Event();watcher=None;reader_thread=None;log=None;started=time.monotonic();serial=0
    def kill_group(sig):
        if process is not None and process.poll() is None:
            try:os.killpg(process.pid,sig)
            except ProcessLookupError:pass
    def watchdog():
        if not finished.wait(110):
            receipt['watchdog_actions'].append('TERM110s');kill_group(signal.SIGTERM)
            if not finished.wait(5):
                receipt['watchdog_actions'].append('KILL115s');kill_group(signal.SIGKILL)
    def interruption(number,_frame):
        raise InterruptedError(f'Signal {number}')
    previous={sig:signal.signal(sig,interruption) for sig in [signal.SIGINT,signal.SIGTERM,signal.SIGHUP]}
    try:
        log=(work/'server.out').open('w')
        process=subprocess.Popen(argv,cwd=work,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
                                 text=True,bufsize=1,start_new_session=True)
        receipt['pid']=process.pid;receipt['process_group']=process.pid
        watcher=threading.Thread(target=watchdog,daemon=True);watcher.start()
        lines=queue.Queue()
        def drain():
            for line in process.stdout:
                log.write(line);log.flush();lines.put(line)
        reader_thread=threading.Thread(target=drain,daemon=True);reader_thread.start()
        def wait(pattern,timeout):
            deadline=min(started+100,time.monotonic()+timeout);received=[]
            while time.monotonic()<deadline:
                if process.poll() is not None:raise RuntimeError('Официальный процесс завершился')
                try:line=lines.get(timeout=min(0.2,max(0.001,deadline-time.monotonic())))
                except queue.Empty:continue
                received.append(line)
                if re.search(pattern,line):return ''.join(received)
            raise TimeoutError('Истёк ограниченный бюджет команды')
        def command(text,timeout=15):
            nonlocal serial
            serial+=1;marker=f'END_SHAPE_SYNC_{serial}_DONE'
            process.stdin.write(text+'\nsay '+marker+'\n');process.stdin.flush()
            reply=wait(marker,timeout)
            receipt['commands'].append({'command':text,'sync_command':'say '+marker,'reply':reply})
            write(args.output/'receipt.json',receipt)
            return reply
        receipt['ready_reply']=wait(r'Done \(',65)
        command('gamerule minecraft:random_tick_speed 0')
        command('gamerule minecraft:spawn_mobs false')
        command('tick freeze')
        x0,z0,x1,z1=plan['forceload_bounds']
        command(f'forceload add {x0} {z0} {x1} {z1}')
        command('forceload query')
        time.sleep(2)
        for x,z in [(x0,z0),(x1,z1),(3200,3200)]:
            marker=f'END_SHAPE_LOADED_{x}_{z}'
            reply=command(f'execute if loaded {x} 120 {z} run say {marker}')
            if '[Server] '+marker not in reply:raise RuntimeError('Загрузка чанка не подтверждена')
        for plot in plan['plots']:
            x,y,z=plot['origin'];reply=command(f'place feature minecraft:end_island {x} {y} {z}')
            receipt['plots'].append({'plot_index':plot['plot_index'],'origin':plot['origin'],'reply':reply})
            if re.search(r'unknown|failed',reply,re.I):raise RuntimeError('Feature не поставлена; серия сохранена как отказ')
        command('save-all flush',20)
        receipt['status']='placed_and_saved'
    except BaseException as exc:
        receipt['error']=type(exc).__name__+': '+str(exc)
    finally:
        # Повторный сигнал не должен прервать остановку собственных процессов.
        for sig in previous:signal.signal(sig,signal.SIG_IGN)
        if process is not None:
            try:
                if process.poll() is None:
                    try:
                        process.stdin.write('stop\n');process.stdin.flush();receipt['stop_command']='stop'
                    except (BrokenPipeError,OSError):pass
                    try:process.wait(timeout=min(10,max(0.1,started+110-time.monotonic())))
                    except subprocess.TimeoutExpired:
                        kill_group(signal.SIGTERM)
                        try:process.wait(timeout=4)
                        except subprocess.TimeoutExpired:kill_group(signal.SIGKILL);process.wait(timeout=3)
            finally:
                receipt['exit_code']=process.poll()
                if process.stdin:process.stdin.close()
        finished.set()
        if watcher:watcher.join(timeout=1)
        if log:
            if reader_thread is not None:reader_thread.join(timeout=2)
            log.close()
        for sig,handler in previous.items():signal.signal(sig,handler)
        receipt['elapsed_seconds']=round(time.monotonic()-started,3)
        try:free_ports(plan);receipt['ports_free_after']=True
        except OSError:receipt['ports_free_after']=False
        write(args.output/'receipt.json',receipt)
    verify_freeze(freeze)
    if receipt['status']!='placed_and_saved' or receipt.get('exit_code')!=0 or not receipt['ports_free_after']:
        raise RuntimeError('Короткий опыт не завершён; receipt сохранён')
    checked=decode(args.output,plan,freeze,receipt)
    print(json.dumps({k:checked[k] for k in ['passed','measured_plots','observed_layers','failed_plots']},ensure_ascii=False))
    if not checked['passed']:raise SystemExit(1)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--freeze',type=Path,required=True)
    parser.add_argument('--jar',type=Path,required=True)
    parser.add_argument('--java',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    run(parser.parse_args())


if __name__=='__main__':
    main()
