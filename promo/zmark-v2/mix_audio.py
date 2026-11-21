"""Build the 35-second soundtrack from the original Galactic Rap MP3."""
import array
import json
import math
from pathlib import Path
import random
import re
import subprocess
import sys
import tempfile
import wave

RATE = 48_000
DURATION = 35
HERE = Path(__file__).resolve().parent
RNG = random.Random(20261121)

# Impact times on the same clock as the four composition timelines.
KEYS = (0.22, 0.32, 0.42, 0.52, 0.62, 1.4, 11.75, 20.5, 22.5, 24.0)
MOVES = (1.5, 4.0, 8.0, 12.0, 14.5, 16.0, 18.0, 19.05, 25.5)
THEMES = (26.5, 27.0, 27.5, 28.0, 28.5, 29.0, 29.5)


def make_effects(path):
    stereo = [array.array("f", [0.0]) * (RATE * DURATION) for _ in range(2)]
    for times, attack, decay, frequency, level, noise in (
        (KEYS, 0.002, 0.018, 980, 0.078, 0.60),
        (MOVES, 0.009, 0.062, 170, 0.105, 0.35),
        (THEMES, 0.004, 0.075, 240, 0.13, 0.16),
        ((31.0,), 0.018, 0.31, 130, 0.17, 0.02),
    ):
        for index, start in enumerate(times):
            offset = round(start * RATE)
            pan = (-0.18, 0.18)[index % 2]
            previous = 0.0
            for sample in range(round((attack + 6 * decay) * RATE)):
                t = sample / RATE
                if t < attack:
                    envelope = t / attack
                else:
                    envelope = math.exp(-(t - attack) / decay)
                white = RNG.uniform(-1, 1)
                tone = math.sin(2 * math.pi * frequency * (t + .055 * (1 - math.exp(-30 * t))))
                effect = level * envelope * (tone * (1 - noise) + (white - previous) * noise)
                previous = white
                stereo[0][offset + sample] += effect * (1 - pan)
                stereo[1][offset + sample] += effect * (1 + pan)
    with wave.open(str(path), "wb") as wav:
        wav.setparams((2, 2, RATE, 0, "NONE", "not compressed"))
        pcm = array.array("h", (round(value * 32767) for frame in zip(*stereo) for value in frame))
        if sys.byteorder != "little":
            pcm.byteswap()
        wav.writeframes(pcm.tobytes())


def mix(source):
    target = "I=-14:TP=-1.5:LRA=7"
    with tempfile.TemporaryDirectory(prefix="zmark-audio-") as directory:
        sfx = Path(directory) / "effects.wav"
        make_effects(sfx)
        command = ["ffmpeg", "-hide_banner", "-nostats", "-y", "-i", str(source), "-i", str(sfx)]
        graph = (
            "[0:a]atrim=start=0.012:duration=35,asetpts=PTS-STARTPTS,"
            "volume='if(between(t,25.7,26.43),0.44,0.88)':eval=frame,"
            "afade=t=in:d=0.045,afade=t=out:st=32.7:d=2.3[bgm];"
            "[bgm][1:a]amix=inputs=2:duration=first:normalize=0,"
        )
        first = subprocess.run(
            command + ["-filter_complex", graph + f"loudnorm={target}:print_format=json",
                       "-ar", str(RATE), "-f", "null", "-"],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
        )
        result = json.loads(re.findall(r"\{[^{}]+\}", first.stderr)[-1])
        measurements = ":".join(
            f"measured_{name}={result[f'input_{name.lower()}']}"
            for name in ("I", "TP", "LRA", "thresh")
        )
        destination = HERE / "assets" / "audio" / "master.m4a"
        subprocess.run(
            command + ["-filter_complex", graph + f"loudnorm={target}:{measurements}:"
                       f"offset={result['target_offset']}:linear=true:print_format=summary",
                       "-ar", str(RATE), "-ac", "2", "-c:a", "aac", "-b:a", "256k",
                       "-metadata", "comment=Galactic Rap by Kevin MacLeod (CC BY 4.0), edited for ZMark",
                       "-movflags", "+faststart", str(destination)],
            check=True,
        )


if __name__ == "__main__":
    mix(Path(sys.argv[1]))
