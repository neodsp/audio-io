#!/usr/bin/env uv run
# /// script
# requires-python = ">=3.9"
# dependencies = [
#     "pyfar",
# ]
# ///

import pyfar as pf

# Parameters
duration = 1  # seconds
sample_rate = 48000  # Hz
frequencies = [440, 554.37, 659.25, 880]  # A4, C#5, E5, A5

# Calculate number of samples
n_samples = int(duration * sample_rate)

# Create 4-channel signal with different sine waves using pyfar
signal = pf.signals.sine(frequencies, n_samples, sampling_rate=sample_rate)

# Write to WAV file
pf.io.write_audio(signal, "sine_4ch_48khz.wav")

print("Generated 4-channel WAV file: sine_4ch_48khz.wav")
print(f"Channels: {signal.cshape}")
print(f"Duration: {signal.n_samples / signal.sampling_rate} seconds")
print(f"Frequencies: {frequencies} Hz")
