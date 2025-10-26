use std::path::Path;

use audio_blocks::AudioBlock;
use hound::{SampleFormat, WavSpec, WavWriter};
use num::Float;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioWriteError {
    #[error("could not decode audio")]
    DecodingError(#[from] hound::Error),
}

/// Sample format for writing audio
#[derive(Debug, Clone, Copy, Default)]
pub enum WriteSampleFormat {
    /// 16-bit integer samples
    #[default]
    Int16,
    /// 32-bit float samples
    Float32,
}

/// Configuration for writing audio to WAV files
#[derive(Default)]
pub struct AudioWriteConfig {
    /// Sample format to use when writing
    pub sample_format: WriteSampleFormat,
}

pub fn audio_write<P: AsRef<Path>, F: Float + 'static>(
    path: P,
    audio_block: impl AudioBlock<F>,
    sample_rate: u32,
    config: AudioWriteConfig,
) -> Result<(), AudioWriteError> {
    let spec = WavSpec {
        channels: audio_block.num_channels(),
        sample_rate,
        bits_per_sample: match config.sample_format {
            WriteSampleFormat::Int16 => 16,
            WriteSampleFormat::Float32 => 32,
        },
        sample_format: match config.sample_format {
            WriteSampleFormat::Int16 => SampleFormat::Int,
            WriteSampleFormat::Float32 => SampleFormat::Float,
        },
    };

    let mut writer = WavWriter::create(path.as_ref(), spec)?;

    match config.sample_format {
        WriteSampleFormat::Int16 => {
            // Convert f32 samples to i16
            for frame in audio_block.frame_iters() {
                for sample in frame {
                    let sample_i16 = (sample.clamp(F::one().neg(), F::one())
                        * F::from(i16::MAX).unwrap_or(F::zero()))
                    .to_i16()
                    .unwrap_or(0);
                    writer.write_sample(sample_i16)?;
                }
            }
        }
        WriteSampleFormat::Float32 => {
            // Write f32 samples directly
            for frame in audio_block.frame_iters() {
                for sample in frame {
                    writer.write_sample(sample.to_f32().unwrap_or(0.0))?;
                }
            }
        }
    }

    writer.finalize()?;

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    #[cfg(all(feature = "read", feature = "write"))]
    fn test_round_trip_i16() {
        use super::*;
        use crate::reader::{AudioReadConfig, audio_read};

        let data1 = audio_read::<_, f32>("test.wav", AudioReadConfig::default()).unwrap();

        audio_write(
            "tmp1.wav",
            data1.audio_block(),
            data1.sample_rate,
            AudioWriteConfig {
                sample_format: WriteSampleFormat::Int16,
            },
        )
        .unwrap();

        let data2 = audio_read::<_, f32>("tmp1.wav", AudioReadConfig::default()).unwrap();
        assert_eq!(data1.sample_rate, data2.sample_rate);
        approx::assert_abs_diff_eq!(
            data1.audio_block().raw_data(),
            data2.audio_block().raw_data(),
            epsilon = 1e-4
        );

        let _ = std::fs::remove_file("tmp1.wav");
    }

    #[test]
    #[cfg(all(feature = "read", feature = "write"))]
    fn test_round_trip_f32() {
        use super::*;
        use crate::reader::{AudioReadConfig, audio_read};

        let data1 = audio_read::<_, f32>("test.wav", AudioReadConfig::default()).unwrap();

        audio_write(
            "tmp2.wav",
            data1.audio_block(),
            data1.sample_rate,
            AudioWriteConfig {
                sample_format: WriteSampleFormat::Float32,
            },
        )
        .unwrap();

        let data2 = audio_read::<_, f32>("tmp2.wav", AudioReadConfig::default()).unwrap();
        assert_eq!(data1.sample_rate, data2.sample_rate);
        approx::assert_abs_diff_eq!(
            data1.audio_block().raw_data(),
            data2.audio_block().raw_data(),
            epsilon = 1e-6
        );

        let _ = std::fs::remove_file("tmp2.wav");
    }
}
