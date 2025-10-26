# audio-io

A simple library to read and write audio files on your disk.

The library can read many formats and can write only to wav files.

## Quick Start

### Read Audio

You can read most common audio formats, if you specify them in the feature flags.

```rs,ignore
use audio_io::*;

let data: AudioData<f32> = audio_read("test.wav", AudioReadConfig::default()).unwrap();
let sample_rate = data.sample_rate;
let block = data.audio_block(); // convert into AudioBlock, which makes it easier to access channels or frames (does not allocate).
```

### Write Audio

You can only write wav files.

```rs,ignore
use audio_io::*;

let sample_rate = 48000
let block = AudioBlockPlanarView::from_slice(&[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]);
audio_write("tmp.wav", block, sample_rate, AudioWriteConfig::default()).unwrap();
```

By leveraging [audio-blocks](https://crates.io/crates/audio-blocks) you can write any audio layout, e.g.:

```rs,ignore
let block = AudioBlockInterleavedView::from_slice(&[0.0, 1.0, 0.0, 1.0, 0.0, 1.0], 2, 3);
audio_write("tmp.wav", block, sample_rate, AudioWriteConfig::default()).unwrap();

let block = AudioBlockSequentialView::from_slice(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], 2, 3);
audio_write("tmp.wav", block, sample_rate, AudioWriteConfig::default()).unwrap();
```

## Supported Input Codecs

Only royalty free codecs are enabled by default.

| Format | Feature Flag | Enabled by Default |
|--------|--------------|-------------------|
| AAC | `aac` | No |
| ADPCM | `adpcm` | Yes |
| ALAC | `alac` | No |
| FLAC | `flac` | Yes |
| CAF | `caf` | No |
| ISO MP4 | `isomp4` | No |
| Matroska (MKV) | `mkv` | Yes |
| MP1 | `mp1` | No |
| MP2 | `mp2` | No |
| MP3 | `mp3` | No |
| Ogg | `ogg` | Yes |
| PCM | `pcm` | Yes |
| AIFF | `aiff` | No |
| Vorbis | `vorbis` | Yes |
| WAV | `wav` | Yes |

To enable all formats, use the `all` feature flag.


## Read and Write Options

### Reading

When reading a file you can specify the following things:

- Start and end in frames or time
- First and last channel

The crate will try to decode and store only the parts that you selected.

### Writing

For writing audio you can only select to store the audio in `Int16` or `Float32`.
By default `Int16` is selected, for broader compatibility.

### Some example configs:

- read exactly 100 frames starting from frame 300
```rs,ignore
audio_read::<_, f32>(
    "test.wav",
    AudioReadConfig {
        start: Start::Frame(300),
        stop: Stop::Frame(400),
        ..Default::default()
    },
```
- read the first 0.5 seconds

```rs,ignore
audio_read::<_, f32>(
    "test.wav",
    AudioReadConfig {
        stop: Stop::Time(std::time::Duration::from_secs_f32(0.5)),
        ..Default::default()
    },
```

- read only the first two channels

```rs,ignore
audio_read::<_, f32>(
    "test.wav",
    AudioReadConfig {
        last_channel: Some(2), // exclusive
        ..Default::default()
    },
```

- skip the first channel

```rs,ignore
audio_read::<_, f32>(
    "test.wav",
    AudioReadConfig {
        first_channel: Some(1),
        ..Default::default()
    },
```

- write audio samples in `f32`

```rs,ignore
audio_write(
    "tmp.wav",
    data1.audio_block(),
    data1.sample_rate,
    AudioWriteConfig {
        sample_format: WriteSampleFormat::Float32,
    },
)
.unwrap();
```
