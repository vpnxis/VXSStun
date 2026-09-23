# Third-party components

VXSStun UI, adapters and profile parser are MIT-licensed. Happ code, binaries,
logos and proprietary assets are not included. The interface is an independent
implementation.

- Xray-core Windows v26.7.28: MPL-2.0, source https://github.com/XTLS/Xray-core/tree/v26.7.28
- libXray Android v26.7.28: MIT, source https://github.com/XTLS/libXray/tree/v26.7.28
  (includes Xray-core and Go dependencies under their respective licenses).
- Wintun 0.14.1: redistribution license in `src-tauri/runtime/LICENSE-Wintun.txt`,
  source https://git.zx2c4.com/wintun/
- Tauri (MIT/Apache-2.0), React (MIT), Vite (MIT): versions recorded in lockfiles.
- jsQR 1.4.0 (Apache-2.0): local QR image decoding, https://github.com/cozmo/jsQR;
  license in LICENSE-jsQR.txt. qrcode 1.5.4 (MIT) is test-only, not bundled in apps.
- AndroidX and Gradle wrapper: Apache-2.0; Kotlin: Apache-2.0.

Unmodified native binaries are obtained from official release URLs by
`scripts/prepare-runtime.ps1`, with pinned SHA-256 checks. Runtime licenses are
included in the Windows package and Android assets. The local debug signing
key is not source material and must never be published.

Before a public store release, collect the full transitive license inventory
for the exact locked dependencies, use a dedicated release signing key, and
complete device/network testing. This alpha is not a security certification.
