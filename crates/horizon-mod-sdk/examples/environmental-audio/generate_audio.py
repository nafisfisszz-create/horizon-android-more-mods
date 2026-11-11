#!/usr/bin/env python3
"""Generate original procedural ambience samples for the mod (requires ffmpeg)."""
import math
import os
import random
import shutil
import struct
import subprocess
import wave

RATE = 24000
SECONDS = 8
COUNT = RATE * SECONDS
ROOT = os.path.join(os.path.dirname(__file__), "assets", "peugaria_audio", "sounds")
AMBIENT = os.path.join(ROOT, "ambient")


def smooth_noise(seed, low=0.0, high=1.0):
    rng = random.Random(seed)
    state = 0.0
    low_state = 0.0
    result = []
    for _ in range(COUNT):
        value = rng.uniform(-1.0, 1.0)
        state += 0.045 * (value - state)
        low_state += 0.004 * (value - low_state)
        band = state - low_state
        result.append(band * high + low_state * low)
    return result


def add_tone(left, right, start, duration, f0, f1, gain, pan=0.0, pulse=None):
    first = int(start * RATE)
    total = max(1, int(duration * RATE))
    for index in range(total):
        at = first + index
        if at >= COUNT:
            break
        phase = index / RATE
        sweep = f0 + (f1 - f0) * phase / duration
        envelope = math.sin(math.pi * min(1.0, phase / duration)) ** 2
        if pulse is not None:
            envelope *= max(0.0, math.sin(2.0 * math.pi * pulse * phase)) ** 6
        sample = gain * envelope * (
            math.sin(2.0 * math.pi * sweep * phase)
            + 0.2 * math.sin(4.02 * math.pi * sweep * phase)
        )
        left[at] += sample * math.sqrt((1.0 - pan) / 2.0)
        right[at] += sample * math.sqrt((1.0 + pan) / 2.0)


def make_scene(kind, seed):
    bed_l = smooth_noise(seed, 0.08, 0.12)
    bed_r = smooth_noise(seed + 1, 0.08, 0.12)
    left = [v * 0.0 for v in bed_l]
    right = [v * 0.0 for v in bed_r]

    if kind == "forest_day":
        for i in range(COUNT):
            gust = 0.55 + 0.45 * math.sin(2 * math.pi * 0.075 * i / RATE)
            left[i] += bed_l[i] * 0.62 * gust
            right[i] += bed_r[i] * 0.62 * gust
        for start, f0, f1, pan in [
            (0.6, 2100, 3300, -0.6), (2.0, 2800, 1900, 0.5),
            (3.3, 1900, 2900, -0.2), (5.4, 2600, 3400, 0.7),
            (6.5, 3200, 2400, -0.5),
        ]:
            add_tone(left, right, start, 0.34, f0, f1, 0.13, pan)
    elif kind == "forest_night":
        for i in range(COUNT):
            left[i] += bed_l[i] * 0.36
            right[i] += bed_r[i] * 0.36
        for start, pan in [(0.6, -0.6), (2.7, 0.55), (4.7, -0.2), (6.3, 0.7)]:
            add_tone(left, right, start, 1.0, 3900, 3935, 0.09, pan, pulse=5.3)
    elif kind == "rain":
        for i in range(COUNT):
            patter = smooth = 0.65 + 0.35 * math.sin(2 * math.pi * 0.13 * i / RATE)
            left[i] += bed_l[i] * 0.95 * patter
            right[i] += bed_r[i] * 0.95 * patter
        rng = random.Random(seed + 5)
        for _ in range(150):
            start = rng.uniform(0.1, 7.8)
            add_tone(left, right, start, 0.018, rng.uniform(1800, 6500),
                     rng.uniform(1200, 4200), rng.uniform(0.015, 0.055),
                     rng.uniform(-0.9, 0.9))
    elif kind == "cave":
        for i in range(COUNT):
            left[i] += bed_l[i] * 0.42
            right[i] += bed_r[i] * 0.42
        for start, pan, freq in [(1.1, -0.5, 780), (3.45, 0.6, 640), (5.9, -0.1, 720)]:
            add_tone(left, right, start, 0.55, freq, freq * 0.42, 0.16, pan)
    elif kind == "beach":
        for i in range(COUNT):
            swell = max(0.0, 0.5 + 0.5 * math.sin(2 * math.pi * 0.075 * i / RATE))
            left[i] += bed_l[i] * (0.18 + 0.68 * swell)
            right[i] += bed_r[i] * (0.18 + 0.68 * swell)
    elif kind == "underwater":
        for i in range(COUNT):
            left[i] += bed_l[i] * 0.55
            right[i] += bed_r[i] * 0.55
        for start, pan, freq in [(0.8, -0.6, 430), (2.5, 0.45, 520),
                                 (4.1, -0.1, 360), (6.2, 0.7, 470)]:
            add_tone(left, right, start, 0.8, freq, freq * 0.62, 0.13, pan)
    elif kind == "nether":
        rumble_l = smooth_noise(seed + 20, 0.8, 0.15)
        rumble_r = smooth_noise(seed + 21, 0.8, 0.15)
        for i in range(COUNT):
            gust = 0.5 + 0.5 * math.sin(2 * math.pi * 0.09 * i / RATE)
            left[i] += rumble_l[i] * 0.8 * gust
            right[i] += rumble_r[i] * 0.8 * gust
        for start in (1.2, 4.8, 6.6):
            add_tone(left, right, start, 0.7, 420, 170, 0.07)
    elif kind == "the_end":
        for i in range(COUNT):
            left[i] += bed_l[i] * 0.28
            right[i] += bed_r[i] * 0.28
        for freq, gain, phase in [(220, 0.035, 0.0), (329.6, 0.025, 1.3),
                                  (440, 0.018, 2.2), (659.3, 0.01, 0.4)]:
            for i in range(COUNT):
                t = i / RATE
                value = gain * math.sin(2 * math.pi * freq * t + phase
                                        + 0.18 * math.sin(2 * math.pi * 0.07 * t))
                left[i] += value
                right[i] += value * math.cos(phase)

    fade = int(0.12 * RATE)
    peak = max(max(abs(v) for v in left), max(abs(v) for v in right), 1e-9)
    scale = 0.24 / peak
    for i in range(COUNT):
        envelope = 1.0
        if i < fade:
            envelope = i / fade
        elif i >= COUNT - fade:
            envelope = (COUNT - 1 - i) / fade
        left[i] = math.tanh(left[i] * scale * envelope * 1.2)
        right[i] = math.tanh(right[i] * scale * envelope * 1.2)
    return left, right


def write_ogg(kind, seed):
    left, right = make_scene(kind, seed)
    os.makedirs(AMBIENT, exist_ok=True)
    wav_path = os.path.join(AMBIENT, kind + ".wav")
    ogg_path = os.path.join(AMBIENT, kind + ".ogg")
    with wave.open(wav_path, "wb") as stream:
        stream.setnchannels(2)
        stream.setsampwidth(2)
        stream.setframerate(RATE)
        frames = bytearray()
        for l, r in zip(left, right):
            frames.extend(struct.pack("<hh",
                int(max(-1.0, min(1.0, l)) * 32767),
                int(max(-1.0, min(1.0, r)) * 32767)))
        stream.writeframes(frames)
    subprocess.run([
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
        "-i", wav_path, "-c:a", "libvorbis", "-q:a", "3", ogg_path
    ], check=True)
    os.remove(wav_path)


def main():
    if shutil.which("ffmpeg") is None:
        raise SystemExit("ffmpeg is required to generate OGG sound assets")
    for index, kind in enumerate([
        "forest_day", "forest_night", "rain", "cave",
        "beach", "underwater", "nether", "the_end",
    ]):
        write_ogg(kind, 20261111 + index * 101)


if __name__ == "__main__":
    main()
