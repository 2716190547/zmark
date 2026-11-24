"""Build the 72-second master: voice-over, ducked music bed and on-screen effects.

Usage: python3 mix_audio.py /path/to/Chill-Wave.mp3
Voice lines are assets/audio/vo/lNN.wav (Kokoro zf_xiaobei, see README.md).
All times are root-film seconds, matching the composition timelines.
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
DURATION = 72.0
HERE = Path(__file__).resolve().parent
RNG = random.Random(20261124)

# Where each voice line starts; anchors are noted in SPEC.md.
VO = [0.6, 6.7, 13.5, 16.9, 23.4, 30.6, 38.5, 44.5, 51.2, 58.9, 63.9]
# Track second that lands on the film's reveal (14.5 s): a downbeat after the break.
MUSIC_OFFSET = 153.635 - 14.5


def typing(at, text_len, step):
    """Key times of typeLine/typeInto: one per character."""
    return [at + i * step for i in range(text_len)], at + text_len * step


def key_times():
    keys = []
    # draft.html (scene at 14.4): lines 0, 2, 4, 5, 6, 7.
    t = 14.4 + 4.0
    for gap, n, step in ((0, 12, 0.06), (0.25, 16, 0.045), (0.25, 9, 0.05), (0.1, 16, 0.03), (0.05, 15, 0.03), (0.05, 14, 0.03)):
        times, t = typing(t + gap, n, step)
        keys += times[::2]
    # split.html (scene at 23.0): the Mermaid fence.
    t = 23.0 + 8.0
    times, t = typing(t, 7, 0.06); keys += times
    times, t = typing(t + 0.1, 10, 0.04); keys += times[::2]
    for n in (12, 19, 11, 19, 12):
        times, t = typing(t + 0.08, n, 0.026); keys += times[::3]
    times, t = typing(t + 0.1, 3, 0.06); keys += times
    # outline.html (scene at 44.4): file name, then the line typed after saving.
    keys += typing(44.4 + 8.12, 7, 0.07)[0]
    keys += typing(44.4 + 10.3, 11, 0.08)[0]
    return keys


PRESSES = [17.5, 23.5, 46.4, 51.8, 59.0]          # shortcut key caps
CLICKS = [38.15, 45.25, 48.3, 53.8]                # cursor clicks
WHOOSH = [14.3, 16.5, 38.3, 43.9, 63.0]            # scene and lightbox moves
PAPER = [5.62] + [7.0 + i * 0.32 for i in range(4)]  # "#" lands, notes arrive
LINES = [8.7 + i * 0.09 for i in range(0, 22, 3)]  # raw text floods in


def make_effects(path):
    n = int(RATE * DURATION)
    out = [array.array('f', [0.0]) * n for _ in range(2)]

    def add(start, length, fn, pan=0.0):
        o = int(start * RATE)
        prev = 0.0
        for i in range(int(length * RATE)):
            if o + i >= n:
                break
            t = i / RATE
            v, prev = fn(t, prev)
            out[0][o + i] += v * (1 - pan)
            out[1][o + i] += v * (1 + pan)

    def tick(level, decay, hp):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            return level * math.exp(-t / decay) * (w - hp * prev), w
        return fn

    def thump(level, freq, decay):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            body = math.sin(2 * math.pi * freq * t * (1 + 0.6 * math.exp(-25 * t)))
            return level * math.exp(-t / decay) * (0.8 * body + 0.2 * w), w
        return fn

    def swoosh(level, length):
        def fn(t, prev):
            w = RNG.uniform(-1, 1)
            env = math.sin(math.pi * min(1, t / length)) ** 2
            v = prev + (w - prev) * 0.18  # soft low-pass noise
            return level * env * v * 2.2, v
        return fn

    for i, t in enumerate(key_times()):
        add(t, 0.05, tick(0.05 * (0.8 + 0.4 * RNG.random()), 0.006, 0.92), (-0.15, 0.15)[i % 2])
    for t in PRESSES:
        add(t, 0.09, tick(0.11, 0.012, 0.7))
        add(t + 0.01, 0.12, thump(0.06, 180, 0.03))
    for t in CLICKS:
        add(t, 0.06, tick(0.12, 0.007, 0.85))
    for i, t in enumerate(WHOOSH):
        add(t - 0.15, 0.4 + 0.08 * (i % 3), swoosh(0.07, 0.4 + 0.08 * (i % 3)), (-0.3, 0.3, 0.0)[i % 3])
    for i, t in enumerate(PAPER):
        if i == 0:
            add(t, 0.4, thump(0.16, 95, 0.08))
        else:
            add(t, 0.25, swoosh(0.045, 0.25), (-0.4, 0.4)[i % 2])
    for t in LINES:
        add(t, 0.05, tick(0.035, 0.008, 0.9))
    # One riser into the dive's impact, then the mark landing.
    def riser(t, prev):
        w = RNG.uniform(-1, 1)
        k = t / 1.5
        v = prev + (w - prev) * (0.04 + 0.5 * k * k)
        return 0.09 * k * k * v * 2.0, v
    add(41.6, 1.5, riser)
    add(43.1, 0.9, thump(0.22, 62, 0.22))
    add(64.15, 0.8, thump(0.15, 80, 0.18))

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
        # Voice: each line levelled to -16 LUFS, placed at its start.
        inputs, parts = [], []
        for i, start in enumerate(VO):
            inputs += ['-i', str(HERE / 'assets' / 'audio' / 'vo' / f'l{i + 1:02d}.wav')]
            ms = round(start * 1000)
            parts.append(f'[{i}:a]aresample={RATE},loudnorm=I=-16:TP=-2:LRA=7,aformat=channel_layouts=stereo,adelay={ms}|{ms}[v{i}]')
        vo = d / 'vo.wav'
        graph = ';'.join(parts) + ';' + ''.join(f'[v{i}]' for i in range(len(VO))) + \
            f'amix=inputs={len(VO)}:normalize=0:duration=longest,apad=whole_dur={DURATION},atrim=0:{DURATION}[out]'
        run(['ffmpeg', '-y', *inputs, '-filter_complex', graph, '-map', '[out]', '-ar', str(RATE), str(vo)])

        graph = (
            f'[0:a]atrim=start={MUSIC_OFFSET:.3f}:duration={DURATION},asetpts=PTS-STARTPTS,aresample={RATE},'
            f'volume=0.42,afade=t=in:d=1.2,afade=t=out:st=69.4:d=2.6[bed];'
            '[1:a]asplit=2[voice][key];'
            '[bed][key]sidechaincompress=threshold=0.03:ratio=6:attack=40:release=450:makeup=1[ducked];'
            '[ducked][voice][2:a]amix=inputs=3:normalize=0:duration=first,'
        )
        target = 'I=-14:TP=-1.5:LRA=9'
        base = ['ffmpeg', '-y', '-i', str(music), '-i', str(vo), '-i', str(sfx)]
        first = run(base + ['-filter_complex', graph + f'loudnorm={target}:print_format=json', '-f', 'null', '-'])
        m = json.loads(re.findall(r'\{[^{}]+\}', first.stderr)[-1])
        measured = ':'.join(f'measured_{k}={m[f"input_{k.lower()}"]}' for k in ('I', 'TP', 'LRA', 'thresh'))
        dest = HERE / 'assets' / 'audio' / 'master.m4a'
        run(base + ['-filter_complex', graph + f'loudnorm={target}:{measured}:offset={m["target_offset"]}:linear=true,aresample={RATE},apad=whole_dur={DURATION}',
                    '-t', str(DURATION),
                    '-ar', str(RATE), '-ac', '2', '-c:a', 'aac', '-b:a', '256k',
                    '-metadata', 'comment=Chill Wave by Kevin MacLeod (CC BY 4.0), edited for ZMark; voice Kokoro zf_xiaobei',
                    '-movflags', '+faststart', str(dest)])
        print(dest)


if __name__ == '__main__':
    mix(Path(sys.argv[1]))
