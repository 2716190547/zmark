"""Build the 55.08-second master: one continuous stretch of music plus on-screen effects.

Usage: python3 mix_audio.py assets/audio/raw/Voxel-Revolution.mp3
Times are film seconds and match index.html (122 BPM, one bar = 1.967 s).

Music: track 68.885–123.965 (28 bars, measured with `hyperframes beats`). It needs no splice:
film 0 is a downbeat; the 3D-free climax hit at film 39.34 is track 108.23, the loudest bar.
"""
import array
import json
import math
import random
import re
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

RATE = 48_000
DURATION = 55.08
B = 1.967
b = B / 4
HERE = Path(__file__).resolve().parent
RNG = random.Random(20261125)
MUSIC_START = 68.885
DUCK = (2 * B, 4 * B - 0.05, 0.45)  # shot 2, the pain point, sits under the bed


def make_effects(path):
    n = int(RATE * DURATION)
    out = [array.array('f', [0.0]) * n for _ in range(2)]

    def add(start, length, fn, pan=0.0):
        o = int(start * RATE)
        prev = 0.0
        for i in range(int(length * RATE)):
            if 0 <= o + i < n:
                v, prev = fn(i / RATE, prev)
                out[0][o + i] += v * (1 - pan)
                out[1][o + i] += v * (1 + pan)

    def tick(level, decay, hp):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            return level * math.exp(-t / decay) * (w - hp * prev), w
        return fn

    def thump(level, freq, decay, sweep=0.6):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            body = math.sin(2 * math.pi * freq * t * (1 + sweep * math.exp(-25 * t)))
            return level * math.exp(-t / decay) * (0.85 * body + 0.15 * w), w
        return fn

    def swoosh(level, length, bright=0.2):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            v = prev + (w - prev) * bright
            return level * math.sin(math.pi * min(1, t / length)) ** 2 * v * 2.2, v
        return fn

    def riser(level, length):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            k = t / length
            v = prev + (w - prev) * (0.03 + 0.55 * k * k)
            return level * k * k * v * 2.0, v
        return fn

    # 1 The cut through the frame on beat 3.
    add(2 * b - 0.12, 0.3, swoosh(0.16, 0.3, 0.35))
    add(2 * b, 0.06, tick(0.2, 0.01, 0.5))
    # 2 The caret in the plain text box.
    for i in range(4):
        add(2 * B + 0.7 + i * 0.9, 0.03, tick(0.03, 0.004, 0.9))
    # 3 The line becomes the Z; the halves lock on the bar.
    add(4 * B + 0.1, 0.8, riser(0.09, 0.8))
    add(5 * B, 0.9, thump(0.36, 55, 0.24, 1.0))
    add(5 * B, 0.07, tick(0.18, 0.012, 0.6))
    # 4–6 Typing: the same segments as SEG in index.html, about 12 keys a second.
    for a, z in ((6 * B + 0.6, 10 * B - 0.5), (10 * B + 0.25, 13 * B - 0.5), (13 * B + 0.25, 13 * B + 3.6)):
        t, i = a, 0
        while t < z:
            add(t, 0.05, tick(0.05 * (0.8 + 0.4 * RNG.random()), 0.006, 0.92), (-0.15, 0.15)[i % 2])
            t += 0.083 * (0.75 + 0.5 * RNG.random())
            i += 1
    add(6 * B, 0.09, tick(0.12, 0.012, 0.7))  # ⌘2 key
    # 6 The diagram renders (RENDER = T6 + 2 bars in index.html).
    add(15 * B, 0.5, thump(0.14, 90, 0.14))
    add(15 * B, 0.25, swoosh(0.06, 0.25, 0.4))
    # 7 One click per palette.
    for i in range(9):
        add(17 * B + i * b, 0.04, tick(0.09, 0.007, 0.85), (-0.3, 0.3)[i % 2])
    # 8 ⌘3 on the bar, then the pull back.
    add(20 * B - 0.5, 0.5, riser(0.08, 0.5))
    add(20 * B, 1.0, thump(0.38, 50, 0.28, 1.0))
    add(20 * B, 0.08, tick(0.2, 0.012, 0.6))
    add(20 * B + 0.5, 1.4, swoosh(0.08, 1.4, 0.12))
    # 9 Facts and 10 the end card, one tick per line.
    for t in (23 * B, 23 * B + b, 23 * B + 2 * b, 25 * B, 25 * B + b, 25 * B + 2 * b, 26 * B):
        add(t, 0.04, tick(0.07, 0.006, 0.9))
    add(25 * B, 0.7, thump(0.2, 62, 0.2))

    with wave.open(str(path), 'wb') as w:
        w.setparams((2, 2, RATE, 0, 'NONE', 'not compressed'))
        pcm = array.array('h', (max(-32767, min(32767, round(v * 32767))) for f in zip(*out) for v in f))
        if sys.byteorder != 'little':
            pcm.byteswap()
        w.writeframes(pcm.tobytes())


def run(cmd):
    return subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)


def mix(music):
    with tempfile.TemporaryDirectory(prefix='zmark-v4-audio-') as d:
        sfx = Path(d) / 'sfx.wav'
        make_effects(sfx)
        a, z, g = DUCK
        bed = (
            f'[0:a]atrim=start={MUSIC_START}:duration={DURATION},asetpts=PTS-STARTPTS,aresample={RATE},'
            f"volume='0.8*if(between(t,{a},{z}),{g},1)':eval=frame,"
            f'afade=t=in:d=0.015,afade=t=out:st={DURATION - 1.4}:d=1.4[bed];'
            '[bed][1:a]amix=inputs=2:normalize=0:duration=first,'
        )
        target = 'I=-14:TP=-1.5:LRA=11'
        base = ['ffmpeg', '-y', '-i', str(music), '-i', str(sfx)]
        first = run(base + ['-filter_complex', bed + f'loudnorm={target}:print_format=json', '-f', 'null', '-'])
        m = json.loads(re.findall(r'\{[^{}]+\}', first.stderr)[-1])
        measured = ':'.join(f'measured_{k}={m[f"input_{k.lower()}"]}' for k in ('I', 'TP', 'LRA', 'thresh'))
        dest = HERE / 'assets' / 'audio' / 'master.m4a'
        run(base + ['-filter_complex', bed + f'loudnorm={target}:{measured}:offset={m["target_offset"]}:linear=true,aresample={RATE},'
                    f'alimiter=limit=0.75:attack=1:release=60:level=disabled,apad=whole_dur={DURATION}',
                    '-t', str(DURATION), '-ar', str(RATE), '-ac', '2', '-c:a', 'aac', '-b:a', '256k',
                    '-metadata', 'comment=Voxel Revolution by Kevin MacLeod (incompetech.com), CC BY 4.0, edited for ZMark',
                    '-movflags', '+faststart', str(dest)])
        print(dest)


if __name__ == '__main__':
    mix(Path(sys.argv[1]))
