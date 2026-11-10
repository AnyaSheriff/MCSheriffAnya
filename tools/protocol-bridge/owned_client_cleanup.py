"""Private probe: retain owned PID/starttime evidence; never relabel zombies gone."""
from pathlib import Path
import os
import signal
import time


def parse_stat(pid, raw):
    fields = raw.rsplit(') ', 1)[1].split()
    if len(fields) < 20 or int(raw.split(' ', 1)[0]) != pid:
        raise ValueError('invalid process identity')
    return dict(pid=pid, state=fields[0], ppid=int(fields[1]), pgid=int(fields[2]),
                sid=int(fields[3]), starttime=int(fields[19]))


def snapshot(owned):
    rows = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            row = parse_stat(int(entry.name), (entry / 'stat').read_text())
        except (FileNotFoundError, ProcessLookupError):
            continue
        rows.append(row)
    identities = {(row['pid'],row['starttime']) for row in owned}
    selected = {row['pid']:row for row in rows if (row['pid'],row['starttime']) in identities}
    while True:
        children = {row['pid']:row for row in rows if row['ppid'] in selected}
        old = len(selected);selected.update(children)
        if len(selected) == old:
            break
    return sorted(selected.values(), key=lambda row: row['pid'])


def same_identity(row):
    try:
        current = parse_stat(row['pid'], Path('/proc', str(row['pid']), 'stat').read_text())
    except (FileNotFoundError, ProcessLookupError):
        return False
    return current['starttime'] == row['starttime']


def pin_owned(client):
    # Called immediately after Popen, before poll/wait can reap and release PID.
    root = parse_stat(client.pid, Path('/proc', str(client.pid), 'stat').read_text())
    if root['sid'] != client.pid or root['pgid'] != client.pid:
        raise ValueError('launch did not create owned session')
    return dict(root=root, initial=snapshot([root]))


def finish_owned(client, stop, pinned):
    if pinned is None:
        raise ValueError('missing original launch identity; cleanup failed without signalling')
    root = pinned['root']
    if root['pid'] != client.pid or root['sid'] != client.pid or not same_identity(root):
        raise ValueError('original launch identity missing/reused before cleanup')
    before = snapshot(pinned['initial'] + [root])
    stop(client)
    after_stop = snapshot(before)
    known = {(row['pid'], row['starttime']): row for row in before + after_stop}
    for action in (signal.SIGTERM, signal.SIGKILL):
        for row in list(known.values()):
            if row['state'] != 'Z' and same_identity(row):
                try:
                    os.kill(row['pid'], action)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            remaining = snapshot(known.values())
            for row in remaining:
                known[(row['pid'], row['starttime'])] = row
            if not remaining:
                break
            time.sleep(.05)
    remaining = snapshot(known.values())
    return dict(launch_identity=pinned, before=before, after_wrapper_stop=after_stop,
                remaining=remaining,live=[row for row in remaining if row['state'] != 'Z'],
                zombies=[row for row in remaining if row['state'] == 'Z'],group_gone=not remaining)
