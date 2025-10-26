#![doc = include_str!("../README.md")]

#[cfg(feature = "read")]
pub use reader::{AudioData, AudioReadConfig, AudioReadError, Start, Stop, audio_read};

#[cfg(feature = "write")]
pub use writer::{AudioWriteConfig, AudioWriteError, audio_write};

pub use audio_blocks::*;

#[cfg(feature = "read")]
pub mod reader;
#[cfg(feature = "write")]
pub mod writer;
