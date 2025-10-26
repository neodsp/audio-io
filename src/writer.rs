use std::{marker::PhantomData, path::Path};

use audio_blocks::AudioBlock;
use hound::{SampleFormat, WavSpec, WavWriter};
use num::Float;

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

#[derive(Default)]
pub struct AudioWriter<F: Float + 'static> {
    _phantom: PhantomData<F>,
}

impl<F: Float> AudioWriter<F> {
    pub fn write<P: AsRef<Path>>(
        path: P,
        audio_block: &impl AudioBlock<F>,
        sample_rate: u32,
        config: AudioWriteConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

        Ok(Self {
            _phantom: PhantomData,
        })
    }
}
