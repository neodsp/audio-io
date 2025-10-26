use std::fs::File;
use std::path::Path;

use audio_blocks::AudioBlockInterleavedView;
use num::Float;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Starting position in the audio stream
#[derive(Debug, Clone, Copy, Default)]
pub enum Start {
    /// Start from the beginning of the audio
    #[default]
    Beginning,
    /// Start at a specific time offset
    Time(std::time::Duration),
    /// Start at a specific frame number (sample position across all channels)
    Frame(usize),
}

/// Ending position in the audio stream
#[derive(Debug, Clone, Copy, Default)]
pub enum Stop {
    /// Read until the end of the audio
    #[default]
    End,
    /// Stop at a specific time offset
    Time(std::time::Duration),
    /// Stop at a specific frame number (sample position across all channels)
    Frame(usize),
}

#[derive(Default)]
pub struct AudioReadConfig {
    /// Where to start reading audio (time or frame-based)
    pub start: Start,
    /// Where to stop reading audio (time or frame-based)
    pub stop: Stop,
    /// First channel to extract (0-indexed). None means start from channel 0.
    pub start_channel: Option<usize>,
    /// Last channel to extract (exclusive). None means extract to the last channel.
    pub end_channel: Option<usize>,
}

#[derive(Default)]
pub struct AudioReader<F: Float + 'static> {
    samples: Vec<F>,
    sample_rate: u32,
    num_channels: usize,
    num_frames: usize,
}

impl<F: Float> AudioReader<F> {
    pub fn read<P: AsRef<Path>>(
        path: P,
        config: AudioReadConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let src = File::open(path.as_ref())?;
        let mss = MediaSourceStream::new(Box::new(src), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.as_ref().extension() {
            if let Some(ext_str) = ext.to_str() {
                hint.with_extension(ext_str);
            }
        }

        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("No audio track found")?;

        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or("Sample rate not specified")?;

        let track_id = track.id;

        // Clone codec params before the mutable borrow
        let codec_params = track.codec_params.clone();
        let time_base = track.codec_params.time_base;

        // Convert Start/Stop to frame numbers
        let start_frame = match config.start {
            Start::Beginning => 0,
            Start::Time(duration) => {
                let secs = duration.as_secs_f64();
                (secs * sample_rate as f64) as usize
            }
            Start::Frame(frame) => frame,
        };

        let end_frame: Option<usize> = match config.stop {
            Stop::End => None,
            Stop::Time(duration) => {
                let secs = duration.as_secs_f64();
                Some((secs * sample_rate as f64) as usize)
            }
            Stop::Frame(frame) => Some(frame),
        };

        // If start_frame is large (more than 1 second), use seeking to avoid decoding everything
        if start_frame > sample_rate as usize {
            if let Some(tb) = time_base {
                // Seek to 90% of the target to account for keyframe positioning
                let seek_sample = (start_frame as f64 * 0.9) as u64;
                let seek_ts = (seek_sample * tb.denom as u64) / (sample_rate as u64);

                // Try to seek, but don't fail if seeking doesn't work
                let _ = format.seek(
                    SeekMode::Accurate,
                    SeekTo::TimeStamp {
                        ts: seek_ts,
                        track_id,
                    },
                );
            }
        }

        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs().make(&codec_params, &dec_opts)?;

        let mut sample_buf = None;
        let mut samples = Vec::new();
        let mut num_channels = 0usize;
        let start_channel = config.start_channel;
        let end_channel = config.end_channel;

        // We'll track exact position by counting samples as we decode
        let mut current_sample: Option<u64> = None;

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(Error::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                }
                Err(err) => return Err(err.into()),
            };

            if packet.track_id() != track_id {
                continue;
            }

            let decoded = decoder.decode(&packet)?;

            // Get the timestamp of this packet to know our position
            if current_sample.is_none() {
                let ts = packet.ts();
                if let Some(tb) = time_base {
                    // Convert timestamp to sample position
                    current_sample = Some((ts * sample_rate as u64) / tb.denom as u64);
                } else {
                    current_sample = Some(0);
                }
            }

            if sample_buf.is_none() {
                let spec = *decoded.spec();
                let duration = decoded.capacity() as u64;
                sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));

                // Get the number of channels from the spec
                num_channels = spec.channels.count();

                // Validate channel range
                if let Some(start_ch) = start_channel {
                    if start_ch >= num_channels {
                        return Err(format!(
                            "Invalid start_channel: {}. Audio has {} channels",
                            start_ch, num_channels
                        )
                        .into());
                    }
                }
                if let Some(end_ch) = end_channel {
                    if end_ch > num_channels {
                        return Err(format!(
                            "Invalid end_channel: {}. Audio has {} channels",
                            end_ch, num_channels
                        )
                        .into());
                    }
                    if let Some(start_ch) = start_channel {
                        if end_ch <= start_ch {
                            return Err(format!(
                                "end_channel ({}) must be greater than start_channel ({})",
                                end_ch, start_ch
                            )
                            .into());
                        }
                    }
                }
            }

            if let Some(buf) = &mut sample_buf {
                buf.copy_interleaved_ref(decoded);
                let packet_samples = buf.samples();

                let mut pos = current_sample.unwrap_or(0);

                // Determine channel range to extract
                let ch_start = start_channel.unwrap_or(0);
                let ch_end = end_channel.unwrap_or(num_channels);
                let num_channels = ch_end - ch_start;

                // Process samples based on whether we're filtering channels
                if ch_start != 0 || ch_end != num_channels {
                    // Channel filtering: samples are interleaved [L, R, L, R, ...] for stereo
                    // We need to extract only the requested channel range
                    let frames = packet_samples.len() / num_channels;

                    for frame_idx in 0..frames {
                        // Check if we've reached the end frame
                        if let Some(end) = end_frame {
                            if pos >= end as u64 {
                                let num_frames = samples.len() / num_channels;
                                return Ok(Self {
                                    sample_rate,
                                    num_channels,
                                    num_frames,
                                    samples,
                                });
                            }
                        }

                        // Start collecting samples once we reach start_frame
                        if pos >= start_frame as u64 {
                            // Extract only the selected channel range from this frame
                            for ch in ch_start..ch_end {
                                let sample_idx = frame_idx * num_channels + ch;
                                samples.push(F::from(packet_samples[sample_idx]).unwrap());
                            }
                        }

                        pos += 1;
                    }
                } else {
                    // No channel filtering: collect all samples
                    let frames = packet_samples.len() / num_channels;

                    for frame_idx in 0..frames {
                        // Check if we've reached the end frame
                        if let Some(end) = end_frame {
                            if pos >= end as u64 {
                                let num_frames = samples.len() / num_channels;
                                return Ok(Self {
                                    sample_rate,
                                    num_channels,
                                    num_frames,
                                    samples,
                                });
                            }
                        }

                        // Start collecting samples once we reach start_frame
                        if pos >= start_frame as u64 {
                            // Collect all channels from this frame
                            for ch in 0..num_channels {
                                let sample_idx = frame_idx * num_channels + ch;
                                samples.push(F::from(packet_samples[sample_idx]).unwrap());
                            }
                        }

                        pos += 1;
                    }
                }

                // Update our position tracker
                current_sample = Some(pos);
            }
        }

        let ch_start = start_channel.unwrap_or(0);
        let ch_end = end_channel.unwrap_or(num_channels);
        let num_channels = ch_end - ch_start;
        let num_frames = samples.len() / num_channels;

        Ok(Self {
            sample_rate,
            num_channels,
            num_frames,
            samples,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn audio_block(&self) -> AudioBlockInterleavedView<'_, F> {
        AudioBlockInterleavedView::from_slice(
            &self.samples,
            self.num_channels as u16,
            self.num_frames,
        )
    }
}

#[test]
fn test_load() {
    let reader = AudioReader::<f32>::read("test.wav", AudioReadConfig::default()).unwrap();
    let block = reader.audio_block();
    dbg!(block);
}
