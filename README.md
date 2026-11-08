# MCSheriffAnya Android core updates

Signed preview channel for the in-app core updater (bootstrap APK code4+).
The app verifies its pinned ECDSA public key, ABI, host API, revision, size and SHA-256 before activation and every launch.
Private signing keys are never stored on GitHub. This branch contains public metadata only.

Core assets are immutable files in the existing v0.2.9-alpha release.
Stop the server before updating; the app creates a data backup. APK updates still use the Android installer.

The first r2 channel validates module replacement with a second full build of the same gameplay source as bundled r1; it does not add gameplay features.
