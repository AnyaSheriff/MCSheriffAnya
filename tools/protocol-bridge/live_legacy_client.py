"""Официальный1.12.2 в отдельном Xvfb; без аккаунтов пользовательского лаунчера."""
import hashlib
import os
import subprocess
import uuid


def launch(cache, root, port, log):
    with (cache / 'client.jar').open('rb') as f:
        assert hashlib.file_digest(f, 'sha1').hexdigest() == '0f275bc1547d01fa5f56ba34bdc87d981ee12daf'
    (root / 'options.txt').write_text('renderDistance:2\nmaxFps:30\nsoundCategory_master:0.0\n')
    identity = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(), version=3)
    command = ['xvfb-run', '-a', '-s', '-screen 0 854x480x24',
               '/usr/lib/jvm/java-8-openjdk/jre/bin/java', '-Xmx1G',
               '-Djava.library.path=' + str(cache / 'natives'),
               '-Dnet.java.games.input.librarypath=' + str(cache / 'natives'),
               '-cp', (cache / 'classpath.txt').read_text().strip(), 'net.minecraft.client.main.Main',
               '--username', 'McsaProbe', '--version', '1.12.2', '--gameDir', str(root),
               '--assetsDir', str(cache / 'assets'), '--assetIndex', '1.12', '--uuid', identity.hex,
               '--accessToken', '0', '--userType', 'legacy', '--versionType', 'release',
               '--width', '854', '--height', '480', '--server', '127.0.0.1', '--port', str(port)]
    environment = os.environ.copy()
    for key in ['WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS']:
        environment.pop(key, None)
    (root / 'runtime').mkdir(mode=0o700)
    environment.update(XDG_SESSION_TYPE='x11', XDG_RUNTIME_DIR=str(root / 'runtime'), ALSOFT_DRIVERS='null')
    return subprocess.Popen(command, cwd=root, stdin=subprocess.DEVNULL, stdout=log,
                            stderr=subprocess.STDOUT, start_new_session=True, env=environment)
