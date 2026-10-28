"""Ограниченное завершение только собственных отдельных групп процессов."""
import os
import signal
import socket
import subprocess
import time
from pathlib import Path


def group_exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False


def process_identity(pid):
    # После ')' идут поля3..; поле22 — время рождения в тактах ядра Linux.
    try:
        fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
        return {'pid': pid, 'pgid': int(fields[2]), 'start_time': fields[19]}
    except FileNotFoundError:
        return None


def owned_members(pid, identity):
    if identity is None or process_identity(pid) != identity:
        return []
    members = []
    for path in Path('/proc').iterdir():
        if path.name.isdecimal():
            try:
                member = process_identity(int(path.name))
            except (OSError, ValueError, IndexError):
                continue
            if member is not None and member['pgid'] == pid:
                members.append(member)
    return members


def signal_group(pid, sig, identity, witnesses=()):
    if pid <= 1 or pid == os.getpgrp():
        raise ValueError('refusing to signal caller process group')
    leader = identity is not None and identity.get('pgid') == pid and process_identity(pid) == identity
    descendant = any(member.get('pgid') == pid and process_identity(member['pid']) == member for member in witnesses)
    if not leader and not descendant:
        raise ValueError('process identity changed; refusing group signal')
    try:
        os.killpg(pid, sig)
    except ProcessLookupError:
        pass


def ports_free(ports):
    result = {}
    for port in ports:
        with socket.socket() as sock:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                sock.bind(('127.0.0.1', port))
                result[str(port)] = True
            except OSError:
                result[str(port)] = False
    return result


def signal_process(proc, sig, identity, witnesses=()):
    if identity is None:
        # Собственный Popen ещё хранится у запускающего. send_signal сначала
        # проверяет состояние своего дочернего процесса; чужой PID из файла
        # таким fallback никогда не обслуживается.
        proc.send_signal(sig)
    else:
        signal_group(proc.pid, sig, identity, witnesses)


def stop_process(proc, control=None, identity=None):
    """Popen обязан иметь start_new_session=True. EOF нужен Tokio stdin."""
    witnesses = owned_members(proc.pid, identity)
    method = 'already-exited'
    if proc.poll() is None:
        try:
            if proc.stdin is not None:
                proc.stdin.write('stop\n')
                proc.stdin.flush()
            elif control is not None:
                control.run('stop')
        except (BrokenPipeError, OSError, EOFError):
            pass
        finally:
            if proc.stdin is not None:
                try:
                    proc.stdin.close()
                except (BrokenPipeError, OSError):
                    pass
        try:
            proc.wait(timeout=8)
            method = 'stop-and-eof'
        except subprocess.TimeoutExpired:
            signal_process(proc, signal.SIGTERM, identity, witnesses)
            try:
                proc.wait(timeout=3)
                method = 'term-after-stop-timeout'
            except subprocess.TimeoutExpired:
                signal_process(proc, signal.SIGKILL, identity, witnesses)
                proc.wait(timeout=2)
                method = 'kill-after-term-timeout'
    # Завершившийся родитель тоже может оставить свою группу потомков.
    if group_exists(proc.pid):
        if proc.poll() is not None and process_identity(proc.pid) is None:
            # Непосредственно удерживаемый Popen собственной start_new_session
            # группы, а не PID из файла. Лидер уже reaped, имя группы удерживают
            # оставшиеся потомки. Новый живой лидер с тем же PID сюда не проходит.
            if proc.pid <= 1 or proc.pid == os.getpgrp():
                raise ValueError('refusing caller process group')
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            signal_process(proc, signal.SIGKILL, identity, witnesses)
        deadline = time.monotonic() + 1
        while group_exists(proc.pid) and time.monotonic() < deadline:
            time.sleep(.02)
    return {'pid': proc.pid, 'exit_code': proc.returncode,
            'method': method, 'group_gone': not group_exists(proc.pid)}
