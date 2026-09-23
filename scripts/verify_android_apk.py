"""Verify an ARM64 APK's native ABI and 16-KiB ZIP/ELF alignment.

Uses only the Python standard library. Signature and manifest checks must also
be run with Android SDK apksigner/aapt; this does not replace those tools.
"""

import argparse
import hashlib
import json
import struct
import zipfile
from pathlib import Path


def verify(apk: Path) -> dict:
    libraries = []
    with apk.open("rb") as raw, zipfile.ZipFile(apk) as archive:
        for entry in archive.infolist():
            if not entry.filename.startswith("lib/") or not entry.filename.endswith(".so"):
                continue
            if not entry.filename.startswith("lib/arm64-v8a/"):
                raise ValueError(f"Unexpected ABI: {entry.filename}")
            if entry.compress_type != zipfile.ZIP_STORED:
                raise ValueError(f"Compressed native library: {entry.filename}")
            raw.seek(entry.header_offset)
            local = raw.read(30)
            if len(local) != 30 or local[:4] != b"PK\x03\x04":
                raise ValueError("Invalid ZIP local header")
            name_size, extra_size = struct.unpack_from("<HH", local, 26)
            data_offset = entry.header_offset + 30 + name_size + extra_size
            if data_offset % 16384:
                raise ValueError(f"ZIP entry is not 16-KiB aligned: {entry.filename}")
            data = archive.read(entry)
            if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01":
                raise ValueError(f"Not a little-endian ELF64 library: {entry.filename}")
            if struct.unpack_from("<H", data, 18)[0] != 183:
                raise ValueError(f"Not AArch64: {entry.filename}")
            phoff = struct.unpack_from("<Q", data, 32)[0]
            phsize, count = struct.unpack_from("<HH", data, 54)
            if phsize < 56 or phoff + phsize * count > len(data):
                raise ValueError("Invalid ELF program header table")
            loads = 0
            for index in range(count):
                kind, _, offset, vaddr, _, _, _, alignment = struct.unpack_from(
                    "<IIQQQQQQ", data, phoff + index * phsize
                )
                if kind != 1:
                    continue
                loads += 1
                if alignment < 16384 or alignment & (alignment - 1) or offset % 16384 != vaddr % 16384:
                    raise ValueError(f"ELF LOAD is not 16-KiB aligned: {entry.filename}")
            if not loads:
                raise ValueError(f"Missing ELF LOAD segments: {entry.filename}")
            libraries.append({"name": entry.filename, "loadSegments": loads, "alignment": 16384})
    names = {Path(item["name"]).name for item in libraries}
    if names != {"libvxsstun_profiles.so", "libgojni.so"}:
        raise ValueError("APK is missing a required native library")
    with apk.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    return {"apk": str(apk.resolve()), "sha256": digest, "bytes": apk.stat().st_size,
            "abi": "arm64-v8a", "nativeAlignment": "passed", "libraries": libraries}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.apk), ensure_ascii=False))
