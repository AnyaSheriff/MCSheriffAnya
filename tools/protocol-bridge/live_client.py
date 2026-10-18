"""Запуск непрозрачного официального клиента только в своём Xvfb."""
import hashlib
import os
import signal
import subprocess
import uuid


def launch(cache, root, port, log):
    assert hashlib.sha1((cache / 'client.jar').read_bytes()).hexdigest() == 'e877b6a07acd633fb3bb475002175cec036e7b87'
    (root / 'options.txt').write_text('onboardAccessibility:false\nskipMultiplayerWarning:true\nnarrator:0\n')
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
               '--username', 'McsaProbe', '--version', '26.3', '--gameDir', str(root),
               '--assetsDir', str(cache / 'assets'), '--assetIndex', '34', '--uuid', identity.hex,
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
