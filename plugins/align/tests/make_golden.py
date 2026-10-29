"""Reference values for the Rust tests of plugins/align (P3): features, resampler, CTC alignment.

Inputs are synthetic: sines plus a 64-bit LCG noise that the Rust tests generate the same way
(src/testdata.rs). Needs numpy, torch, torchaudio and transformers (the P0 environment):

  python make_golden.py golden.json
"""
import json
import sys

import numpy as np
import torch
import torchaudio
import torchaudio.functional as F
from transformers import SeamlessM4TFeatureExtractor

OUT = sys.argv[1]
M = 2**64


class Lcg:
    def __init__(self, seed):
        self.x = seed % M

    def next(self):
        self.x = (self.x * 6364136223846793005 + 1442695040888963407) % M
        return (self.x >> 11) / float(2**53)


def signal(n, sr, seed):
    rng = Lcg(seed)
    t = np.arange(n) / sr
    noise = np.array([2 * rng.next() - 1 for _ in range(n)])
    return (0.3 * np.sin(2 * np.pi * 220 * t) + 0.3 * np.sin(2 * np.pi * 1000 * t) + 0.3 * noise).astype(np.float32)


golden = {}

# Features: 0.5 s at 16 kHz (47 frames, odd, so the padding path runs too) and 1 s.
fe = SeamlessM4TFeatureExtractor()
for n in (7900, 16000):
    x = signal(n, 16000, 7)
    out = fe(x, sampling_rate=16000, return_tensors="np")
    f = out["input_features"][0]
    m = out["attention_mask"][0]
    golden[f"features_{n}"] = {
        "shape": list(f.shape),
        "mask": m.tolist(),
        "rows": {str(i): [round(float(v), 5) for v in f[i]] for i in (0, f.shape[0] // 2, f.shape[0] - 1)},
        "col_mean": [round(float(v), 5) for v in f.mean(0)],
    }

# Resampler (torchaudio defaults: sinc_interp_hann, width 6, rolloff 0.99).
for sr in (44100, 48000):
    x = signal(sr // 2, sr, 11)
    y = torchaudio.functional.resample(torch.from_numpy(x), sr, 16000).numpy()
    golden[f"resample_{sr}"] = {
        "len": int(y.shape[0]),
        "head": [round(float(v), 6) for v in y[:16]],
        "mid": [round(float(v), 6) for v in y[3000:3016]],
        "tail": [round(float(v), 6) for v in y[-16:]],
    }

# CTC forced alignment on random emissions.
rng = Lcg(3)
T, V = 60, 7
em = torch.tensor([[rng.next() * 4 for _ in range(V)] for _ in range(T)], dtype=torch.float32)
em = torch.log_softmax(em, dim=-1)
targets = [1, 3, 2, 2, 5, 1, 6, 4, 1]
labels, scores = F.forced_align(em[None], torch.tensor([targets], dtype=torch.int32), blank=0)
spans = F.merge_tokens(labels[0], scores[0].exp())
golden["ctc"] = {"T": T, "V": V, "seed": 3, "scale": 4, "targets": targets, "spans": [[s.token, s.start, s.end] for s in spans]}

json.dump(golden, open(OUT, "w"), indent=1)
print("features", golden["features_7900"]["shape"], golden["features_16000"]["shape"])
print("resample", golden["resample_44100"]["len"], golden["resample_48000"]["len"])
print("ctc spans", golden["ctc"]["spans"])
