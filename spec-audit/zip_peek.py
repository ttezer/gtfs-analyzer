#!/usr/bin/env python3
"""Read a ZIP's FILE LIST without downloading it in full (HTTP Range, with curl).

The ZIP central directory is at the END of the file. Fetching the last ~64 KB to find the EOCD
record, reading the directory block offset from it, and making a second Range request for only
that block when needed is sufficient — 57 feeds × ~100 KB instead of downloading everything.

⚠️ Use `curl`, NOT `urllib`: this machine has no Python certificate store
(`extract_provisions.py` docstring'i aynı tuzağı not ediyor).
⚠️ If the server does not support Range, SKIP that feed and report it as such —
do not CONFUSE "file absent" with "could not inspect".
"""
import struct, subprocess, sys


def _curl(url, rng, timeout=90, max_bytes=52428800):
    p = subprocess.run(
        ["curl", "-sSL", "--max-time", str(timeout), "--max-filesize", str(max_bytes),
         "-r", rng, "-w", "\n%{http_code}", url],
        capture_output=True,
    )
    if p.returncode != 0:
        raise RuntimeError(f"curl failed: {p.stderr.decode()[:80]}")
    body, _, code = p.stdout.rpartition(b"\n")
    return code.decode().strip(), body


def filenames(url, tail=65536):
    code, buf = _curl(url, f"-{tail}")
    # Most servers ignore Range and return the FULL body with 200. In that case we already
    # have the entire ZIP — it has been downloaded, so there is no point in discarding it.
    # (Size is limited by `--max-filesize`; curl errors when exceeded and the feed becomes "uninspectable".)
    if code not in ("206", "200"):
        raise RuntimeError(f"HTTP {code}")
    if code == "200" and not buf.startswith(b"PK"):
        raise RuntimeError("not a ZIP (200 response without a PK signature)")
    i = buf.rfind(b"PK\x05\x06")
    if i < 0:
        raise RuntimeError("EOCD not found (not a ZIP or tail is too short)")
    cd_size, cd_off = struct.unpack("<II", buf[i + 12:i + 20])
    if cd_off == 0xFFFFFFFF:
        raise RuntimeError("ZIP64 is not supported")
    # The tail's start position in the file is unknown; if the directory fits in the tail,
    # find its signature directly, otherwise make a second request.
    j = buf.find(b"PK\x01\x02")
    if j >= 0 and len(buf) - j >= cd_size:
        cd = buf[j:j + cd_size]
    else:
        code2, cd = _curl(url, f"{cd_off}-{cd_off + cd_size - 1}")
        if code2 != "206":
            raise RuntimeError(f"could not fetch central directory block (HTTP {code2})")
    out, p = [], 0
    while p + 46 <= len(cd) and cd[p:p + 4] == b"PK\x01\x02":
        nlen, elen, clen = struct.unpack("<HHH", cd[p + 28:p + 34])
        out.append(cd[p + 46:p + 46 + nlen].decode("utf-8", "replace"))
        p += 46 + nlen + elen + clen
    if not out:
        raise RuntimeError("could not parse central directory")
    return out


if __name__ == "__main__":
    print(filenames(sys.argv[1]))
