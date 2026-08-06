#!/usr/bin/env python3
"""Zip'in DOSYA LİSTESİNİ tam indirmeden oku (HTTP Range, curl ile).

Zip'in merkezî dizini dosyanın SONUNDADIR. Son ~64 KB'ı çekip EOCD kaydını bulmak,
oradan dizin bloğunun ofsetini okuyup gerekirse ikinci bir Range isteğiyle yalnız o
bloğu almak yeterli — 57 feed × ~100 KB, tamamını indirmek yerine.

⚠️ `urllib` DEĞİL `curl`: bu makinede Python'un sertifika deposu yok
(`extract_provisions.py` docstring'i aynı tuzağı not ediyor).
⚠️ Sunucu Range desteklemiyorsa o feed ATLANIR ve öyle raporlanır —
"dosya yok" ile "bakılamadı" KARIŞTIRILMAZ.
"""
import struct, subprocess, sys


def _curl(url, rng, timeout=90, max_bytes=52428800):
    p = subprocess.run(
        ["curl", "-sSL", "--max-time", str(timeout), "--max-filesize", str(max_bytes),
         "-r", rng, "-w", "\n%{http_code}", url],
        capture_output=True,
    )
    if p.returncode != 0:
        raise RuntimeError(f"curl hatası: {p.stderr.decode()[:80]}")
    body, _, code = p.stdout.rpartition(b"\n")
    return code.decode().strip(), body


def filenames(url, tail=65536):
    code, buf = _curl(url, f"-{tail}")
    # Sunucuların çoğu Range'i yok sayıp TAM gövdeyi 200 ile döndürüyor. O hâlde
    # elimizde zaten bütün zip var — indirme yapılmış, atmanın anlamı yok.
    # (Boyut `--max-filesize` ile sınırlı; aşarsa curl hata verir ve feed "bakılamadı"ya düşer.)
    if code not in ("206", "200"):
        raise RuntimeError(f"HTTP {code}")
    if code == "200" and not buf.startswith(b"PK"):
        raise RuntimeError("zip değil (200, PK imzası yok)")
    i = buf.rfind(b"PK\x05\x06")
    if i < 0:
        raise RuntimeError("EOCD yok (zip değil ya da kuyruk kısa)")
    cd_size, cd_off = struct.unpack("<II", buf[i + 12:i + 20])
    if cd_off == 0xFFFFFFFF:
        raise RuntimeError("zip64 — desteklenmiyor")
    # Kuyruğun dosya içindeki başlangıcı bilinmiyor; dizin kuyruğa sığıyorsa
    # imzayı doğrudan arayarak buluruz, sığmıyorsa ikinci istek atarız.
    j = buf.find(b"PK\x01\x02")
    if j >= 0 and len(buf) - j >= cd_size:
        cd = buf[j:j + cd_size]
    else:
        code2, cd = _curl(url, f"{cd_off}-{cd_off + cd_size - 1}")
        if code2 != "206":
            raise RuntimeError(f"dizin bloğu alınamadı (HTTP {code2})")
    out, p = [], 0
    while p + 46 <= len(cd) and cd[p:p + 4] == b"PK\x01\x02":
        nlen, elen, clen = struct.unpack("<HHH", cd[p + 28:p + 34])
        out.append(cd[p + 46:p + 46 + nlen].decode("utf-8", "replace"))
        p += 46 + nlen + elen + clen
    if not out:
        raise RuntimeError("dizin ayrıştırılamadı")
    return out


if __name__ == "__main__":
    print(filenames(sys.argv[1]))
