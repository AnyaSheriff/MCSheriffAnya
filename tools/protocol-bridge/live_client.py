"""Запуск непрозрачного официального клиента только в своём Xvfb."""
import hashlib
import json
import os
import signal
import subprocess
import uuid


CLIENT_SHA1 = {"1.21.4": "a7e5a6024bfd3cd614625aa05629adf760020304", "1.21.5": "b88808bbb3da8d9f453694b5d8f74a3396f1a533", "1.21.6": "740a125b83dd3447feaa3c5e891ead7fbb21ae28", "1.21.8": "a19d9badbea944a4369fd0059e53bf7286597576", "1.21.10": "d3bdf582a7fa723ce199f3665588dcfe6bf9aca8", "1.21.11": "ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd", "26.2": "2dc72797acbc1b63fc16a11c4ac393605f453754",
               "26.3": "e877b6a07acd633fb3bb475002175cec036e7b87"}


def launch(cache, root, port, log, *, version="26.3", gui_scale=None, pause_on_lost_focus=None):
    assert version in CLIENT_SHA1
    metadata = json.loads((cache / 'version.json').read_text())
    assert metadata['id'] == version
    assert metadata['downloads']['client']['sha1'] == CLIENT_SHA1[version]
    assert hashlib.sha1((cache / 'client.jar').read_bytes()).hexdigest() == CLIENT_SHA1[version]
    options = 'onboardAccessibility:false\nskipMultiplayerWarning:true\nnarrator:0\n'
    if gui_scale is not None:
        if type(gui_scale) is not int or not 1 <= gui_scale <= 4:
            raise ValueError('invalid private GUI scale')
        options += f'guiScale:{gui_scale}\n'
    if pause_on_lost_focus is not None:
        if type(pause_on_lost_focus) is not bool:
            raise ValueError('invalid pause option')
        options += 'pauseOnLostFocus:' + str(pause_on_lost_focus).lower() + '\n'
    (root / 'options.txt').write_text(options)
    identity = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(), version=3)
    command = ['xvfb-run', '-a', '-s', '-screen 0 854x480x24',
               '/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-XX:StackShadowPages=32',
               '--enable-native-access=ALL-UNNAMED', '--add-exports', 'java.base/jdk.internal.misc=ALL-UNNAMED',
               '-Djava.library.path=' + str(root / 'natives/java'),
               '-Djna.tmpdir=' + str(root / 'natives/jna'),
               '-Dorg.lwjgl.system.SharedLibraryExtractPath=' + str(root / 'natives/lwjgl'),
               '-Dio.netty.native.workdir=' + str(root / 'natives/netty'),
               '-Dminecraft.launcher.brand=mcsa-probe', '-Dminecraft.launcher.version=1',
               '-cp', (cache / 'classpath.txt').read_text().strip(), 'net.minecraft.client.main.Main',
               '--username', 'McsaProbe', '--version', version, '--gameDir', str(root),
               '--assetsDir', str(cache / 'assets'), '--assetIndex', metadata['assetIndex']['id'], '--uuid', identity.hex,
               '--accessToken', '0', '--clientId', '0', '--xuid', '0', '--versionType', 'release',
               '--width', '854', '--height', '480', '--quickPlayMultiplayer', f'127.0.0.1:{port}']
    environment = os.environ.copy()
    for key in ['WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS']:
        environment.pop(key, None)
    (root / 'runtime').mkdir(mode=0o700)
    environment.update(XDG_SESSION_TYPE='x11', XDG_RUNTIME_DIR=str(root / 'runtime'), ALSOFT_DRIVERS='null')
    return subprocess.Popen(command, cwd=root, stdin=subprocess.DEVNULL, stdout=log,
                            stderr=subprocess.STDOUT, start_new_session=True, env=environment)


def stop(client):
    try:
        os.killpg(client.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        client.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(client.pid, signal.SIGKILL)
        client.wait()
