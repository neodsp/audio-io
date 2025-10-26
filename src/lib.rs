#[cfg(feature = "read")]
pub mod reader;
#[cfg(feature = "write")]
pub mod writer;

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::time::Duration;

//     #[test]
//     fn test_read_full_audio() {
//         // Read the entire audio file
//         let audio = read_audio("test.wav", AudioReadConfig::default()).unwrap();
//         dbg!(audio.sample_rate);
//         dbg!(audio.num_channels);
//         dbg!(audio.data.len());
//     }

//     #[test]
//     fn test_read_audio_with_frame_range() {
//         // Read frames from 1000 to 5000
//         let config = AudioReadConfig {
//             start: Start::Frame(1000),
//             stop: Stop::Frame(5000),
//             ..Default::default()
//         };
//         let audio = read_audio("test.wav", config).unwrap();
//         dbg!(audio.sample_rate);
//         dbg!(audio.num_channels);
//         dbg!(audio.data.len());
//         // Should have 4000 frames
//         assert_eq!(audio.num_frames, 4000);
//         assert_eq!(audio.data.len(), 4000 * audio.num_channels);
//     }

//     #[test]
//     fn test_read_audio_with_time_range() {
//         // Read audio from 0.5 seconds to 1.5 seconds
//         let config = AudioReadConfig {
//             start: Start::Time(Duration::from_secs_f64(0.5)),
//             stop: Stop::Time(Duration::from_secs_f64(1.5)),
//             ..Default::default()
//         };
//         let audio = read_audio("test.wav", config).unwrap();
//         dbg!(audio.sample_rate);
//         dbg!(audio.num_channels);
//         dbg!(audio.data.len());
//         // Should be approximately 1 second of audio
//         let expected_frames = audio.sample_rate as usize; // 1 second worth of frames
//         assert_eq!(audio.num_frames, expected_frames);
//         assert_eq!(audio.data.len(), expected_frames * audio.num_channels);
//     }

//     #[test]
//     fn test_read_left_channel_only() {
//         // Extract only the left channel (channel 0) from stereo audio
//         let config = AudioReadConfig {
//             start: Start::Frame(1000),
//             stop: Stop::Frame(5000),
//             start_channel: Some(0),
//             end_channel: Some(1), // Exclusive, so this extracts only channel 0
//         };
//         let audio = read_audio("test.wav", config).unwrap();
//         dbg!(audio.sample_rate);
//         dbg!(audio.num_channels);
//         dbg!(audio.data.len());
//         assert_eq!(audio.num_channels, 1); // Should have 1 output channel
//         assert_eq!(audio.num_frames, 4000); // 4000 frames
//         assert_eq!(audio.data.len(), 4000); // 4000 frames * 1 channel
//     }

//     #[test]
//     fn test_read_right_channel_only() {
//         // Extract only the right channel (channel 1) from stereo audio
//         let config = AudioReadConfig {
//             start_channel: Some(1),
//             end_channel: Some(2), // Exclusive, so this extracts only channel 1
//             ..Default::default()
//         };
//         let audio = read_audio("test.wav", config).unwrap();
//         dbg!(audio.sample_rate);
//         dbg!(audio.num_channels);
//         dbg!(audio.data.len());
//         assert_eq!(audio.num_channels, 1); // Should have 1 output channel
//     }

//     #[test]
//     fn test_read_channel_range() {
//         // For 5.1 surround (6 channels), extract channels 2-4
//         // This would extract center, LFE, and surround left
//         let config = AudioReadConfig {
//             start_channel: Some(2),
//             end_channel: Some(5), // Exclusive, extracts channels 2, 3, 4
//             ..Default::default()
//         };
//         // Note: This test will fail on stereo audio, but demonstrates the API
//         // Uncomment if you have a 5.1 audio file:
//         // let audio = read_audio("surround.wav", config).unwrap();
//         // assert_eq!(audio.num_channels, 3); // Should have 3 output channels
//     }

//     #[test]
//     fn test_combined_time_and_channel_selection() {
//         // Extract left channel only from a 2-second segment starting at 1 second
//         let config = AudioReadConfig {
//             start: Start::Time(Duration::from_secs(1)),
//             stop: Stop::Time(Duration::from_secs(3)),
//             start_channel: Some(0),
//             end_channel: Some(1),
//         };
//         let audio = read_audio("test.wav", config).unwrap();
//         dbg!(audio.sample_rate);
//         dbg!(audio.num_channels);
//         dbg!(audio.data.len());
//         assert_eq!(audio.num_channels, 1);
//         // Should be approximately 2 seconds of audio in mono
//         let expected_frames = audio.sample_rate as usize * 2;
//         assert_eq!(audio.num_frames, expected_frames);
//         assert_eq!(audio.data.len(), expected_frames);
//     }

//     #[test]
//     fn test_write_audio_int16() {
//         // Create test audio data
//         let audio = AudioData::new(44100, 2, 100, vec![0.5; 200]);

//         // Write as int16
//         let config = AudioWriteConfig {
//             sample_format: WriteSampleFormat::Int16,
//         };
//         write_audio("test_output_i16.wav", &audio, config).unwrap();

//         // Read it back and verify
//         let read_back = read_audio("test_output_i16.wav", AudioReadConfig::default()).unwrap();
//         assert_eq!(read_back.sample_rate, 44100);
//         assert_eq!(read_back.num_channels, 2);
//         assert_eq!(read_back.num_frames, 100);

//         // Clean up
//         let _ = std::fs::remove_file("test_output_i16.wav");
//     }

//     #[test]
//     fn test_write_audio_float32() {
//         // Create test audio data with varying samples
//         let mut data = Vec::new();
//         for i in 0..200 {
//             data.push((i as f32 / 200.0) * 0.5);
//         }
//         let audio = AudioData::new(48000, 2, 100, data);

//         // Write as float32
//         let config = AudioWriteConfig {
//             sample_format: WriteSampleFormat::Float32,
//         };
//         write_audio("test_output_f32.wav", &audio, config).unwrap();

//         // Read it back and verify
//         let read_back = read_audio("test_output_f32.wav", AudioReadConfig::default()).unwrap();
//         assert_eq!(read_back.sample_rate, 48000);
//         assert_eq!(read_back.num_channels, 2);
//         assert_eq!(read_back.num_frames, 100);

//         // Clean up
//         let _ = std::fs::remove_file("test_output_f32.wav");
//     }

//     #[test]
//     fn test_roundtrip() {
//         // Read original, write it, then read it back
//         let original = read_audio("test.wav", AudioReadConfig::default()).unwrap();

//         write_audio("test_roundtrip.wav", &original, AudioWriteConfig::default()).unwrap();

//         let roundtrip = read_audio("test_roundtrip.wav", AudioReadConfig::default()).unwrap();

//         assert_eq!(original.sample_rate, roundtrip.sample_rate);
//         assert_eq!(original.num_channels, roundtrip.num_channels);
//         assert_eq!(original.num_frames, roundtrip.num_frames);

//         // Clean up
//         let _ = std::fs::remove_file("test_roundtrip.wav");
//     }
// }
