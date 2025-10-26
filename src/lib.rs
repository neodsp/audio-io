use std::fs::File;
use std::path::Path;
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

pub fn read_audio(
    path: impl AsRef<Path>,
    config: AudioReadConfig,
) -> Result<(u32, usize, Vec<f32>), Box<dyn std::error::Error>> {
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
            let output_channels = ch_end - ch_start;

            // Process samples based on whether we're filtering channels
            if ch_start != 0 || ch_end != num_channels {
                // Channel filtering: samples are interleaved [L, R, L, R, ...] for stereo
                // We need to extract only the requested channel range
                let frames = packet_samples.len() / num_channels;

                for frame_idx in 0..frames {
                    // Check if we've reached the end frame
                    if let Some(end) = end_frame {
                        if pos >= end as u64 {
                            return Ok((sample_rate, output_channels, samples));
                        }
                    }

                    // Start collecting samples once we reach start_frame
                    if pos >= start_frame as u64 {
                        // Extract only the selected channel range from this frame
                        for ch in ch_start..ch_end {
                            let sample_idx = frame_idx * num_channels + ch;
                            samples.push(packet_samples[sample_idx]);
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
                            return Ok((sample_rate, num_channels, samples));
                        }
                    }

                    // Start collecting samples once we reach start_frame
                    if pos >= start_frame as u64 {
                        // Collect all channels from this frame
                        for ch in 0..num_channels {
                            let sample_idx = frame_idx * num_channels + ch;
                            samples.push(packet_samples[sample_idx]);
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
    let output_channels = ch_end - ch_start;
    Ok((sample_rate, output_channels, samples))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_read_full_audio() {
        // Read the entire audio file
        let (sr, channels, audio) = read_audio("test.wav", AudioReadConfig::default()).unwrap();
        dbg!(sr);
        dbg!(channels);
        dbg!(audio.len());
    }

    #[test]
    fn test_read_audio_with_frame_range() {
        // Read frames from 1000 to 5000
        let config = AudioReadConfig {
            start: Start::Frame(1000),
            stop: Stop::Frame(5000),
            ..Default::default()
        };
        let (sr, channels, audio) = read_audio("test.wav", config).unwrap();
        dbg!(sr);
        dbg!(channels);
        dbg!(audio.len());
        // 4000 frames * number of channels
        assert_eq!(audio.len(), 4000 * channels);
    }

    #[test]
    fn test_read_audio_with_time_range() {
        // Read audio from 0.5 seconds to 1.5 seconds
        let config = AudioReadConfig {
            start: Start::Time(Duration::from_secs_f64(0.5)),
            stop: Stop::Time(Duration::from_secs_f64(1.5)),
            ..Default::default()
        };
        let (sr, channels, audio) = read_audio("test.wav", config).unwrap();
        dbg!(sr);
        dbg!(channels);
        dbg!(audio.len());
        // Should be approximately 1 second of audio
        let expected_frames = sr as usize; // 1 second worth of frames
        assert_eq!(audio.len(), expected_frames * channels);
    }

    #[test]
    fn test_read_left_channel_only() {
        // Extract only the left channel (channel 0) from stereo audio
        let config = AudioReadConfig {
            start: Start::Frame(1000),
            stop: Stop::Frame(5000),
            start_channel: Some(0),
            end_channel: Some(1), // Exclusive, so this extracts only channel 0
        };
        let (sr, channels, audio) = read_audio("test.wav", config).unwrap();
        dbg!(sr);
        dbg!(channels);
        dbg!(audio.len());
        assert_eq!(channels, 1); // Should have 1 output channel
        assert_eq!(audio.len(), 4000); // 4000 frames * 1 channel
    }

    #[test]
    fn test_read_right_channel_only() {
        // Extract only the right channel (channel 1) from stereo audio
        let config = AudioReadConfig {
            start_channel: Some(1),
            end_channel: Some(2), // Exclusive, so this extracts only channel 1
            ..Default::default()
        };
        let (sr, channels, audio) = read_audio("test.wav", config).unwrap();
        dbg!(sr);
        dbg!(channels);
        assert_eq!(channels, 1); // Should have 1 output channel
    }

    #[test]
    fn test_read_channel_range() {
        // For 5.1 surround (6 channels), extract channels 2-4
        // This would extract center, LFE, and surround left
        let config = AudioReadConfig {
            start_channel: Some(2),
            end_channel: Some(5), // Exclusive, extracts channels 2, 3, 4
            ..Default::default()
        };
        // Note: This test will fail on stereo audio, but demonstrates the API
        // Uncomment if you have a 5.1 audio file:
        // let (sr, channels, audio) = read_audio("surround.wav", config).unwrap();
        // assert_eq!(channels, 3); // Should have 3 output channels
    }

    #[test]
    fn test_combined_time_and_channel_selection() {
        // Extract left channel only from a 2-second segment starting at 1 second
        let config = AudioReadConfig {
            start: Start::Time(Duration::from_secs(1)),
            stop: Stop::Time(Duration::from_secs(3)),
            start_channel: Some(0),
            end_channel: Some(1),
        };
        let (sr, channels, audio) = read_audio("test.wav", config).unwrap();
        dbg!(sr);
        dbg!(channels);
        dbg!(audio.len());
        assert_eq!(channels, 1);
        // Should be approximately 2 seconds of audio in mono
        let expected_frames = sr as usize * 2;
        assert_eq!(audio.len(), expected_frames);
    }
}
