#!/usr/bin/env python3
"""Confirm only a focused, system-owned Android package installer; never adb install."""
import json
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
trusted = {'com.android.packageinstaller', 'com.google.android.packageinstaller', 'com.android.permissioncontroller', 'com.google.android.permissioncontroller'}

def adb(*args):
    with (out / 'ui-commands.jsonl').open('a') as log:
        log.write(json.dumps(['adb', *args]) + '\n')
    return subprocess.check_output(['adb', *args], timeout=30)

try:
    clicked = False
    for attempt in range(30):
        focus = adb('shell', 'dumpsys', 'window', 'windows').decode(errors='replace')
        (out / f'window-{attempt:02}.txt').write_text(focus)
        focused = re.findall(r'mCurrentFocus=.*? ([\w.]+)/', focus)
        if not focused:
            focused = re.findall(r'mFocusedApp=.*? ([\w.]+)/', focus)
        adb('shell', 'uiautomator', 'dump', '/sdcard/mcsa-install.xml')
        xml = adb('shell', 'cat', '/sdcard/mcsa-install.xml')
        (out / f'ui-{attempt:02}.xml').write_bytes(xml)
        (out / f'ui-{attempt:02}.png').write_bytes(adb('exec-out', 'screencap', '-p'))
        if focused and focused[-1] in trusted:
            package = focused[-1]
            info = adb('shell', 'dumpsys', 'package', package).decode(errors='replace')
            (out / f'installer-{package}.txt').write_text(info)
            if not re.search(r'(?:pkgFlags|flags)=\[[^\]]*\bSYSTEM\b', info):
                raise RuntimeError('focused installer is not a system package')
            root = ET.fromstring(xml)
            for node in root.iter('node'):
                if node.get('package') != package or node.get('enabled') != 'true' or node.get('clickable') != 'true':
                    continue
                # Exact positive action only: never Open/Done, permissions, or unknown dialogs.
                if node.get('text', '').strip().casefold() not in {'install', 'update', 'установить', 'обновить'}:
                    continue
                bounds = re.fullmatch(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]', node.get('bounds', ''))
                if not bounds:
                    continue
                x1, y1, x2, y2 = map(int, bounds.groups())
                adb('shell', 'input', 'tap', str((x1+x2)//2), str((y1+y2)//2))
                (out / 'system-confirmation.json').write_text(json.dumps({'package': package, 'text': node.get('text'), 'bounds': node.get('bounds'), 'system_package': True}, indent=2)+'\n')
                clicked = True
                break
        if clicked:
            break
        time.sleep(1)
    if not clicked:
        raise RuntimeError('recognized focused system installer confirmation not found')
    for attempt in range(90):
        state = adb('shell', 'dumpsys', 'package', 'ru.sheriffanya.mcsa').decode(errors='replace')
        (out / 'package-after.txt').write_text(state)
        if re.search(r'\bversionCode=7\b', state):
            break
        time.sleep(1)
    else:
        raise RuntimeError('system-confirmed update did not reach versionCode 7')
except Exception as error:
    (out / 'failure.json').write_text(json.dumps({'status': 'failed', 'error': str(error), 'adb_install_fallback': False}, indent=2)+'\n')
    raise
