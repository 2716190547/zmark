"""Build the 58.8-second master: music bed and on-screen effects, no voice.

Usage: python3 mix_audio.py /path/to/EDM-Detection-Mode.mp3
All times are root-film seconds, matching the composition timelines.

Music edit (128 BPM, one bar = 1.875 s):
  film 0.0–51.5  <- track 121.0–172.5: the build, then the drop at 135.0 lands on film 14.0
  film 51.5–58.8 <- track 356.25–363.55: the outro; its last hit at 358.125 lands on film 53.375
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
DURATION = 58.8
HERE = Path(__file__).resolve().parent
RNG = random.Random(20261124)
BAR = 1.875
SPLICE = 51.5
A_START, B_START = 121.0, 356.25


def typing(at, n, step):
    return [at + i * step for i in range(n)], at + n * step


def key_times():
    keys = []
    t = 14.0 + 4.1  # draft.html
    for gap, n, step in ((0, 12, 0.045), (0.15, 16, 0.03), (0.15, 9, 0.04), (0.08, 16, 0.022), (0.04, 15, 0.022), (0.04, 14, 0.022)):
        times, t = typing(t + gap, n, step)
        keys += times[::2]
    t = 21.5 + 5.4  # split.html, the Mermaid fence
    times, t = typing(t, 7, 0.04); keys += times[::2]
    times, t = typing(t + 0.05, 10, 0.03); keys += times[::2]
    for n in (12, 19, 11, 19, 12):
        times, t = typing(t + 0.05, n, 0.018); keys += times[::3]
    keys += typing(37.25 + 3.1, 7, 0.06)[0]  # outline.html, file name
    keys += typing(37.25 + 4.3, 11, 0.06)[0][::2]  # the line typed after saving
    return keys


PRESSES = [17.75, 21.95, 38.375, 39.9, 44.45]   # ⌘N ⌘2 ⌘⇧O ⌘S ⌘3
CLICKS = [30.8, 37.6, 39.15, 41.1]               # expand, Drawer, outline item, 存储
WHOOSH = [6.5, 6.73, 6.97, 7.2, 15.6, 30.9, 36.95, 47.75]
SWIPES = [[0.05, 0.3, 0.55, 0.8, 1.05], [-0.5, 0.4, -0.3, 0.6, 0.0]]  # storm pass-bys and their pans
TITLES = [2.75, 6.5, 8.375, 17.75, 21.55, 27.125, 31.1, 38.2, 39.9, 44.3]
DUCKS = [(32.28, 32.75, 0.25), (13.6, 14.0, 0.55)]  # (from, to, gain): a breath before a hit


def make_effects(path):
    n = int(RATE * DURATION)
    out = [array.array('f', [0.0]) * n for _ in range(2)]

    def add(start, length, fn, pan=0.0):
        o = int(start * RATE)
        prev = 0.0
        for i in range(int(length * RATE)):
            if o + i >= n:
                break
            if o + i < 0:
                continue
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

    def swoosh(level, length, bright=0.18):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            env = math.sin(math.pi * min(1, t / length)) ** 2
            v = prev + (w - prev) * bright
            return level * env * v * 2.2, v
        return fn

    def riser(level, length):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            k = t / length
            v = prev + (w - prev) * (0.03 + 0.55 * k * k)
            return level * k * k * v * 2.0, v
        return fn

    # The storm: pass-bys, then the slam on the beat at 1.8125 s.
    for t, pan in zip(*SWIPES):
        add(t, 0.32, swoosh(0.11, 0.32, 0.25), pan)
    add(1.25, 0.56, riser(0.10, 0.56))
    add(1.8125, 0.9, thump(0.42, 52, 0.26, 1.2))
    add(1.8125, 0.08, tick(0.25, 0.012, 0.6))
    for i, t in enumerate(key_times()):
        add(t, 0.05, tick(0.06 * (0.8 + 0.4 * RNG.random()), 0.006, 0.92), (-0.15, 0.15)[i % 2])
    for t in PRESSES:
        add(t, 0.09, tick(0.13, 0.012, 0.7))
        add(t + 0.01, 0.14, thump(0.08, 170, 0.035))
    for t in CLICKS:
        add(t, 0.06, tick(0.14, 0.007, 0.85))
    for i, t in enumerate(WHOOSH):
        add(t - 0.12, 0.34 + 0.06 * (i % 3), swoosh(0.08, 0.34 + 0.06 * (i % 3)), (-0.35, 0.35, 0.0)[i % 3])
    for t in TITLES:  # a soft tick under each title's first character
        add(t, 0.04, tick(0.05, 0.005, 0.95))
    for i in range(22):  # raw text flooding the page
        if i % 3 == 0:
            add(8.375 + i * 0.06, 0.04, tick(0.045, 0.006, 0.9))
    add(13.52, 0.4, swoosh(0.12, 0.4, 0.3))  # everything pulled into the centre
    # The dive: in on the bar at 32.75, out on the bar at 36.5.
    add(31.9, 0.85, riser(0.09, 0.85))
    add(32.75, 0.7, thump(0.26, 60, 0.2))
    add(36.5, 0.6, thump(0.2, 75, 0.16))
    add(41.0, 0.05, tick(0.06, 0.008, 0.6))  # the save sheet closes
    add(43.25, 0.12, thump(0.05, 420, 0.03, 0.1))  # auto-save lands
    # The finale: the page bursts, pieces swirl in, the Z locks on the music's last hit.
    add(47.75, 0.7, thump(0.2, 70, 0.18))
    for i in range(8):
        add(49.3 + i * 0.5, 0.3, swoosh(0.05, 0.3, 0.3), (-0.5 + (i % 4) * 0.33))
    add(51.6, 1.775, riser(0.08, 1.775))
    add(53.375, 1.2, thump(0.34, 48, 0.3, 1.0))
    add(53.82, 0.42, swoosh(0.07, 0.42))  # the icon glides into the lockup

    with wave.open(str(path), 'wb') as w:
        w.setparams((2, 2, RATE, 0, 'NONE', 'not compressed'))
        pcm = array.array('h', (max(-32767, min(32767, round(v * 32767))) for f in zip(*out) for v in f))
        if sys.byteorder != 'little':
            pcm.byteswap()
        w.writeframes(pcm.tobytes())


def run(cmd):
    return subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)


def mix(music):
    with tempfile.TemporaryDirectory(prefix='zmark-v3-audio-') as d:
        d = Path(d)
        sfx = d / 'sfx.wav'
        make_effects(sfx)
        duck = '*'.join(f"if(between(t,{a},{b}),{g},1)" for a, b, g in DUCKS)
        tail = DURATION - SPLICE
        bed = (
            f'[0:a]atrim=start={A_START}:duration={SPLICE + 0.03},asetpts=PTS-STARTPTS,aresample={RATE}[a];'
            f'[0:a]atrim=start={B_START - 0.03}:duration={tail + 0.03},asetpts=PTS-STARTPTS,aresample={RATE}[b];'
            '[a][b]acrossfade=d=0.06:c1=tri:c2=tri,'
            f"volume='0.8*{duck}':eval=frame,afade=t=in:d=0.02,afade=t=out:st={DURATION - 0.8}:d=0.8[bed];"
            '[bed][1:a]amix=inputs=2:normalize=0:duration=first,'
        )
        target = 'I=-14:TP=-1.5:LRA=11'
        base = ['ffmpeg', '-y', '-i', str(music), '-i', str(sfx)]
        first = run(base + ['-filter_complex', bed + f'loudnorm={target}:print_format=json', '-f', 'null', '-'])
        m = json.loads(re.findall(r'\{[^{}]+\}', first.stderr)[-1])
        measured = ':'.join(f'measured_{k}={m[f"input_{k.lower()}"]}' for k in ('I', 'TP', 'LRA', 'thresh'))
        dest = HERE / 'assets' / 'audio' / 'master.m4a'
        run(base + ['-filter_complex', bed + f'loudnorm={target}:{measured}:offset={m["target_offset"]}:linear=true,aresample={RATE},alimiter=limit=0.7:attack=1:release=60:level=disabled,apad=whole_dur={DURATION}',
                    '-t', str(DURATION), '-ar', str(RATE), '-ac', '2', '-c:a', 'aac', '-b:a', '256k',
                    '-metadata', 'comment=EDM Detection Mode by Kevin MacLeod (CC BY 4.0), edited for ZMark',
                    '-movflags', '+faststart', str(dest)])
        print(dest)


if __name__ == '__main__':
    mix(Path(sys.argv[1]))
