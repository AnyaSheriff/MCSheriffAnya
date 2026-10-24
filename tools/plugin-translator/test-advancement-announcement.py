#!/usr/bin/env python3
"""Проверка единственного объявления достижения в чате и консоли без плагинов."""
from pathlib import Path
import hashlib
import json
import signal
import socket
import subprocess
import tempfile
import time
import uuid
from build_guard import wait_for_idle

repo = Path(__file__).resolve().parents[2]
port = 26223
(repo / 'tmp').mkdir(exist_ok=True)
with socket.socket() as probe:
    if probe.connect_ex(('127.0.0.1', port)) == 0:
        raise SystemExit('Test port occupied')
identity = bytearray(hashlib.md5(b'OfflinePlayer:AdvTester').digest())
identity[6] = (identity[6] & 15) | 48
identity[8] = (identity[8] & 63) | 128
bot = r'''
const assert=require('node:assert/strict');
let mineflayer;try{mineflayer=require(process.cwd()+'/tools/blackbox/node_modules/mineflayer');}catch{mineflayer=require(process.env.HOME+'/node_modules/mineflayer');}
const bot=mineflayer.createBot({host:'127.0.0.1',port:26223,username:'AdvTester',version:'26.1',auth:'offline'});
const messages=[];bot.on('messagestr',line=>messages.push(line));
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const timer=setTimeout(()=>{console.error(messages);process.exit(1)},30000);
bot.on('error',e=>{console.error(e);process.exit(1)});
bot.once('spawn',async()=>{try{
 await pause(1000);
 for(const id of ['adventure/kill_a_mob','story/smelt_iron']){
  bot.chat('/advancement grant AdvTester only minecraft:'+id);await pause(800);
  bot.chat('/advancement grant AdvTester only minecraft:'+id);await pause(800);
 }
 for(const title of ['Monster Hunter','Acquire Hardware'])
  assert.equal(messages.filter(s=>s.includes('has made the advancement ['+title+']')).length,1,JSON.stringify(messages));
 clearTimeout(timer);console.log('PASS: two advancements, one chat announcement each; repeated grants do not announce');
 bot.quit();setTimeout(()=>process.exit(0),300);
}catch(e){console.error(e);process.exit(1)}});
'''
wait_for_idle()
with tempfile.TemporaryDirectory(prefix='advancement-once-', dir=repo / 'tmp') as folder:
    work = Path(folder)
    (work / 'config').mkdir()
    (work / 'config/server.properties').write_text(f'server-port={port}\nonline-mode=false\nlevel-type=minecraft\\:flat\nview-distance=3\nsimulation-distance=3\n')
    (work / 'config/mcsa.properties').write_text('auth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\nconsole-colors=never\nsend-error-reports=false\n')
    (work / 'ops.json').write_text(json.dumps([{'uuid':str(uuid.UUID(bytes=bytes(identity))), 'name':'AdvTester', 'level':4}]))
    log = work / 'console.log'
    with log.open('w') as output:
        server = subprocess.Popen(['nice','-n','19',str(repo / 'target/release/mcsheriffanya')], cwd=work, stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT)
        try:
            for _ in range(1200):
                if server.poll() is not None:
                    raise RuntimeError(log.read_text())
                if 'Готово (' in log.read_text():
                    break
                time.sleep(.1)
            else:
                raise RuntimeError('Startup timeout')
            subprocess.run(['nice','-n','19','node','-'], input=bot, text=True, cwd=repo, timeout=45, check=True)
            time.sleep(.5)
            for title in ['Monster Hunter','Acquire Hardware']:
                lines = [line for line in log.read_text().splitlines() if f'AdvTester has made the advancement [{title}]' in line]
                assert len(lines) == 1, (title, lines)
            print('PASS: two advancements, one console announcement each; no plugins loaded', flush=True)
        finally:
            server.send_signal(signal.SIGINT)
            try:
                server.wait(timeout=30)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=10)
