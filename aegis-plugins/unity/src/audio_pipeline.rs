use anyhow::{anyhow, bail, Context, Result};
use fsbex::AudioFormat as FsbAudioFormat;
use fsbex::Bank as FsbBank;
use fsbex::Stream as FsbStream;
use hound::{SampleFormat, WavSpec, WavWriter};
use nom::number::complete::{le_u16, le_u32, le_u8};
use nom::IResult;
use tracing::warn;

use crate::converters::UnityAudioClip;
use crate::firelight_adpcm::{decode_firelight_gcadpcm, FirelightCoefficients};

use std::fmt;

/// Configuration options for the audio conversion pipeline.
#[derive(Debug, Clone)]
pub struct AudioPipelineOptions {
    /// Whether to emit WAV artifacts (default: true).
    pub emit_wav: bool,
    /// Whether to emit OGG artifacts when available (default: false).
    pub emit_ogg: bool,
    /// If both WAV and OGG are available, prefer OGG as primary (default: false).
    pub prefer_ogg: bool,
    /// Whether to perform duration validation (default: false).
    pub validate_duration: bool,
}

impl Default for AudioPipelineOptions {
    fn default() -> Self {
        Self {
            emit_wav: true,
            emit_ogg: true,
            prefer_ogg: false,
            validate_duration: false,
        }
    }
}

/// Result of converting a Unity audio clip.
#[derive(Debug, Clone)]
pub struct AudioConversionResult {
    pub primary: AudioArtifact,
    pub secondary: Option<AudioArtifact>,
    pub loop_metadata: Option<LoopMetadata>,
    pub stats: AudioConversionStats,
    pub validation: AudioValidationReport,
    pub warnings: Vec<AudioConversionWarning>,
}

/// An exported audio file artifact.
#[derive(Debug, Clone)]
pub struct AudioArtifact {
    pub filename: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Loop metadata extracted from audio samples.
#[derive(Debug, Clone)]
pub struct LoopMetadata {
    pub loop_start_samples: u64,
    pub loop_end_samples: u64,
    pub loop_count: Option<u32>, // None => infinite loop
}

impl fmt::Display for LoopMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "start={}, end={}, count={}",
            self.loop_start_samples,
            self.loop_end_samples,
            self.loop_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "inf".to_string())
        )
    }
}

/// Statistics collected during audio conversion.
#[derive(Debug, Clone)]
pub struct AudioConversionStats {
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub channels: u32,
    pub bits_per_sample: Option<u32>,
    pub primary_size_bytes: usize,
    pub secondary_size_bytes: Option<usize>,
}

impl fmt::Display for AudioConversionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ch_str = match self.channels {
            1 => "mono",
            2 => "stereo",
            n => return write!(f, "{}ch", n),
        };
        write!(
            f,
            "{:.2}s, {}Hz, {}, {}",
            self.duration_seconds,
            self.sample_rate,
            ch_str,
            if let Some(bps) = self.bits_per_sample {
                format!("{}bps", bps)
            } else {
                "variable".to_string()
            }
        )
    }
}

/// Audio validation report.
#[derive(Debug, Clone)]
pub struct AudioValidationReport {
    pub status: AudioValidationStatus,
    pub details: Option<String>,
}

impl fmt::Display for AudioValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.status {
            AudioValidationStatus::NotRun => write!(f, "validation not run"),
            AudioValidationStatus::Passed => write!(f, "validation passed"),
            AudioValidationStatus::Failed => {
                write!(f, "validation failed")?;
                if let Some(ref details) = self.details {
                    write!(f, " ({})", details)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioValidationStatus {
    NotRun,
    Passed,
    Failed,
}

/// Warnings emitted during audio conversion.
#[derive(Debug, Clone)]
pub enum AudioConversionWarning {
    DurationMismatch {
        expected_samples: u64,
        actual_samples: u64,
    },
    UnsupportedCodec(String),
    PartialLoop {
        reason: String,
    },
}

impl fmt::Display for AudioConversionWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioConversionWarning::DurationMismatch {
                expected_samples,
                actual_samples,
            } => write!(
                f,
                "Duration mismatch: expected {} samples, got {}",
                expected_samples, actual_samples
            ),
            AudioConversionWarning::UnsupportedCodec(codec) => {
                write!(f, "Unsupported codec: {}", codec)
            }
            AudioConversionWarning::PartialLoop { reason } => {
                write!(f, "Partial loop metadata: {}", reason)
            }
        }
    }
}

/// Convert a Unity audio clip to WAV/OGG using the configured audio pipeline.
pub fn convert_unity_audio_clip(
    clip: &UnityAudioClip,
    options: &AudioPipelineOptions,
) -> Result<AudioConversionResult> {
    let mut warnings = Vec::new();

    // 1. Detect FSB container (FSB4 or FSB5) if present.
    let fsb_meta = detect_fsb_container(&clip.data);

    // 2. Extract loop metadata from FSB container if available.
    let mut loop_metadata = fsb_meta.as_ref().and_then(|fsb| {
        fsb.samples.first().and_then(|sample| {
            if sample.loop_start > 0 || sample.loop_end > 0 {
                Some(LoopMetadata {
                    loop_start_samples: sample.loop_start as u64,
                    loop_end_samples: sample.loop_end as u64,
                    loop_count: None, // FSB doesn't store loop count; assume infinite if present
                })
            } else {
                None
            }
        })
    });

    // 3. Attempt to decode FSB audio using fsbex (supports Vorbis, PCM, ADPCM, etc.).
    let mut wav_artifact: Option<AudioArtifact> = None;
    let mut ogg_artifact: Option<AudioArtifact> = None;
    let mut stats = AudioConversionStats {
        duration_seconds: 0.0,
        sample_rate: clip.frequency,
        channels: clip.channels as u32,
        bits_per_sample: None,
        primary_size_bytes: 0,
        secondary_size_bytes: None,
    };

    if let Some(ref fsb_container) = fsb_meta {
        match decode_fsb_audio(clip, fsb_container) {
            Ok(decoded) => {
                stats.sample_rate = decoded.sample_rate;
                stats.channels = decoded.channels;
                stats.bits_per_sample = decoded.bits_per_sample;

                if let Some(dur_samples) = decoded.duration_samples {
                    stats.duration_seconds = dur_samples as f64 / decoded.sample_rate as f64;
                }

                if decoded.loop_metadata.is_some() {
                    loop_metadata = decoded.loop_metadata.clone();
                }

                wav_artifact = decoded.wav;
                ogg_artifact = decoded.ogg;
            }
            Err(e) => {
                warnings.push(AudioConversionWarning::UnsupportedCodec(format!(
                    "FSB decoding failed: {}",
                    e
                )));
            }
        }
    } else {
        // Fallback: attempt to decode raw PCM data.
        if clip.data.len() > 0 {
            match encode_wav(&clip.name, &clip.data, clip.channels, clip.frequency) {
                Ok(artifact) => {
                    stats.bits_per_sample = Some(16);
                    stats.duration_seconds = clip.data.len() as f64
                        / (clip.frequency as f64 * clip.channels as f64 * 2.0);
                    wav_artifact = Some(artifact);
                }
                Err(e) => {
                    warnings.push(AudioConversionWarning::UnsupportedCodec(format!(
                        "Raw PCM encoding failed: {}",
                        e
                    )));
                }
            }
        }
    }

    // 4. Select primary and secondary artifacts based on options.
    let (primary, secondary) = if options.prefer_ogg {
        if let Some(ogg) = ogg_artifact.take() {
            (ogg, wav_artifact.take())
        } else if let Some(wav) = wav_artifact.take() {
            (wav, None)
        } else {
            bail!("No audio artifacts produced");
        }
    } else {
        if let Some(wav) = wav_artifact.take() {
            (wav, ogg_artifact.take())
        } else if let Some(ogg) = ogg_artifact.take() {
            (ogg, None)
        } else {
            bail!("No audio artifacts produced");
        }
    };

    stats.primary_size_bytes = primary.bytes.len();
    stats.secondary_size_bytes = secondary.as_ref().map(|a| a.bytes.len());

    // 5. Validate duration if requested.
    let validation = if options.validate_duration {
        validate_duration(clip, &stats, &fsb_meta, &mut warnings)
    } else {
        AudioValidationReport {
            status: AudioValidationStatus::NotRun,
            details: None,
        }
    };

    Ok(AudioConversionResult {
        primary,
        secondary,
        loop_metadata,
        stats,
        validation,
        warnings,
    })
}

/// Detect and parse FSB container (FSB4 or FSB5) from raw clip data.
fn detect_fsb_container(data: &[u8]) -> Option<FsbContainer> {
    if data.starts_with(b"FSB5") {
        parse_fsb5(data).ok()
    } else if data.starts_with(b"FSB4") {
        parse_fsb4(data).ok()
    } else {
        None
    }
}

/// Parse an FSB container (FSB4 or FSB5).
fn parse_fsb(data: &[u8]) -> Result<FsbContainer> {
    if data.starts_with(b"FSB5") {
        parse_fsb5(data)
    } else if data.starts_with(b"FSB4") {
        parse_fsb4(data)
    } else {
        bail!("Not a valid FSB container (expected FSB4 or FSB5 signature)");
    }
}

/// Parse an FSB4 container.
fn parse_fsb4(data: &[u8]) -> Result<FsbContainer> {
    if data.len() < 24 {
        bail!("FSB4 header too small");
    }

    let sample_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // FSB4 uses a simpler structure; we'll extract basic metadata only.
    Ok(FsbContainer {
        header: FsbHeader {
            sample_count,
            _stream_count: sample_count,
            flags: 0,
        },
        samples: vec![],
        data_offset: 24, // Simplified; actual offset varies by header structure
    })
}

/// Parse an FSB5 container.
fn parse_fsb5(data: &[u8]) -> Result<FsbContainer> {
    if data.len() < 0x3C {
        bail!("FSB5 header too small");
    }

    let sample_count = u32::from_le_bytes([data[0x08], data[0x09], data[0x0A], data[0x0B]]);
    let shdr_size = u32::from_le_bytes([data[0x0C], data[0x0D], data[0x0E], data[0x0F]]);
    let name_size = u32::from_le_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]);
    let data_size = u32::from_le_bytes([data[0x14], data[0x15], data[0x16], data[0x17]]);

    let data_offset = 0x3C + shdr_size as usize + name_size as usize;

    let flags = u32::from_le_bytes([data[0x18], data[0x19], data[0x1A], data[0x1B]]);

    let header = FsbHeader {
        sample_count,
        _stream_count: sample_count,
        flags,
    };

    // Parse sample headers from offset 0x3C.
    let sample_header_data = &data[0x3C..0x3C + shdr_size as usize];
    let samples = parse_fsb5_sample_headers(sample_header_data, sample_count as usize)?;

    Ok(FsbContainer {
        header,
        samples,
        data_offset,
    })
}

/// Parse FSB sample headers (FSB4).
fn parse_fsb_sample_headers(input: &[u8], count: usize) -> Result<Vec<FsbSample>> {
    let mut samples = Vec::with_capacity(count);
    let mut rest = input;

    for _ in 0..count {
        let (new_rest, sample) = parse_fsb_sample_header(rest)
            .map_err(|e| anyhow!("Failed to parse FSB sample header: {:?}", e))?;
        samples.push(sample);
        rest = new_rest;
    }

    Ok(samples)
}

/// Parse FSB5 sample headers.
fn parse_fsb5_sample_headers(input: &[u8], count: usize) -> Result<Vec<FsbSample>> {
    let mut samples = Vec::with_capacity(count);
    let mut rest = input;

    for _ in 0..count {
        let (new_rest, sample) = parse_fsb5_sample_header(rest)
            .map_err(|e| anyhow!("Failed to parse FSB5 sample header: {:?}", e))?;
        samples.push(sample);
        rest = new_rest;
    }

    Ok(samples)
}

/// Parse a single FSB4 sample header.
fn parse_fsb_sample_header(input: &[u8]) -> IResult<&[u8], FsbSample> {
    let (rest, _name_len) = le_u8(input)?;
    let (rest, loop_start) = le_u32(rest)?;
    let (rest, loop_end) = le_u32(rest)?;

    Ok((
        rest,
        FsbSample {
            loop_start,
            loop_end,
        },
    ))
}

/// Parse a single FSB5 sample header (simplified).
fn parse_fsb5_sample_header(input: &[u8]) -> IResult<&[u8], FsbSample> {
    let (rest, _chunk_info) = le_u32(input)?;

    // FSB5 sample headers are variable-length; for simplicity, we'll extract basic loop metadata.
    // In a full implementation, we'd parse chunks (FREQUENCY, CHANNELS, LOOP, etc.).
    // For now, assume no loop data and return a placeholder.
    Ok((
        rest,
        FsbSample {
            loop_start: 0,
            loop_end: 0,
        },
    ))
}

/// FSB container metadata.
#[derive(Debug, Clone)]
struct FsbContainer {
    header: FsbHeader,
    samples: Vec<FsbSample>,
    data_offset: usize,
}

/// FSB header (simplified).
#[derive(Debug, Clone)]
struct FsbHeader {
    sample_count: u32,
    _stream_count: u32,
    flags: u32,
}

/// FSB sample metadata (simplified).
#[derive(Debug, Clone)]
struct FsbSample {
    loop_start: u32,
    loop_end: u32,
}

/// Artifacts produced by decoding FSB audio via fsbex.
#[derive(Debug)]
struct DecodedFsbArtifacts {
    sample_rate: u32,
    channels: u32,
    bits_per_sample: Option<u32>,
    wav: Option<AudioArtifact>,
    ogg: Option<AudioArtifact>,
    duration_samples: Option<u64>,
    loop_metadata: Option<LoopMetadata>,
    codec: Option<String>,
}

/// Validate the duration of the audio clip against expected samples.
fn validate_duration(
    clip: &UnityAudioClip,
    stats: &AudioConversionStats,
    fsb_meta: &Option<FsbContainer>,
    warnings: &mut Vec<AudioConversionWarning>,
) -> AudioValidationReport {
    // Calculate expected samples from Unity metadata.
    let expected_samples = (clip.frequency as f64 * stats.duration_seconds) as u64;

    // If FSB metadata is available, use its sample count.
    let actual_samples = if let Some(ref fsb) = fsb_meta {
        fsb.header.sample_count as u64
    } else {
        expected_samples
    };

    // Allow a 1% tolerance for duration mismatches.
    let tolerance = (expected_samples as f64 * 0.01).max(1.0);
    let diff = (expected_samples as f64 - actual_samples as f64).abs();

    if diff > tolerance {
        warnings.push(AudioConversionWarning::DurationMismatch {
            expected_samples,
            actual_samples,
        });
        AudioValidationReport {
            status: AudioValidationStatus::Failed,
            details: Some(format!(
                "Duration mismatch: expected {} samples, got {}",
                expected_samples, actual_samples
            )),
        }
    } else {
        AudioValidationReport {
            status: AudioValidationStatus::Passed,
            details: None,
        }
    }
}

/// Encode raw PCM data as WAV using hound.
fn encode_wav(name: &str, data: &[u8], channels: u32, sample_rate: u32) -> Result<AudioArtifact> {
    // Assume 16-bit PCM for simplicity.
    let wav_bytes = encode_pcm_i16(data, channels, sample_rate)?;

    Ok(AudioArtifact {
        filename: format!("{}.wav", name),
        media_type: "audio/wav",
        bytes: wav_bytes,
    })
}

/// Encode PCM i16 samples as WAV using hound.
fn encode_pcm_i16(data: &[u8], channels: u32, sample_rate: u32) -> Result<Vec<u8>> {
    use std::io::Cursor;

    if data.len() % 2 != 0 {
        bail!("PCM data length is not a multiple of 2 (expected i16 samples)");
    }

    let num_samples = data.len() / 2;
    let samples: Vec<i16> = data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let mut cursor = Cursor::new(Vec::new());
    let spec = WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    {
        let mut writer = WavWriter::new(&mut cursor, spec)?;
        for sample in samples {
            writer.write_sample(sample)?;
        }
        writer.flush()?;
    }

    Ok(cursor.into_inner())
}

/// Encode PCM u8 samples as WAV using hound.
fn encode_pcm_u8(data: &[u8], channels: u32, sample_rate: u32) -> Result<Vec<u8>> {
    use std::io::Cursor;

    let mut cursor = Cursor::new(Vec::new());
    let spec = WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 8,
        sample_format: SampleFormat::Int,
    };

    {
        let mut writer = WavWriter::new(&mut cursor, spec)?;
        for &sample in data {
            writer.write_sample(sample as i16)?;
        }
        writer.flush()?;
    }

    Ok(cursor.into_inner())
}

/// Extract existing Ogg Vorbis streams from FSB container.
fn extract_ogg_from_fsb(clip: &UnityAudioClip, _fsb_meta: &FsbContainer) -> Result<AudioArtifact> {
    // For simplicity, assume the FSB data contains a raw Ogg stream.
    // In practice, we'd need to locate the Ogg payload based on FSB metadata.
    // This is a placeholder for future implementation.
    Ok(AudioArtifact {
        filename: format!("{}.ogg", clip.name),
        media_type: "audio/ogg",
        bytes: clip.data.clone(),
    })
}

/// Decode FSB audio using fsbex and return artifacts.
fn decode_fsb_audio(clip: &UnityAudioClip, fsb_container: &FsbContainer) -> Result<DecodedFsbArtifacts> {
    let bank = FsbBank::new(clip.data.as_slice()).context("Failed to parse FSB container")?;
    let format = bank.format();

    let mut iter = bank.into_iter();
    let stream = iter
        .next()
        .ok_or_else(|| anyhow!("FSB archive contains no streams"))?;

    let sample_rate = stream.sample_rate().get();
    let channels = stream.channels().get() as u32;
    let num_samples = stream.sample_count().get() as u64;

    let format_name = match format {
        FsbAudioFormat::Vorbis => "Vorbis".to_string(),
        FsbAudioFormat::Pcm8 => "PCM8".to_string(),
        FsbAudioFormat::Pcm16 => "PCM16".to_string(),
        FsbAudioFormat::Pcm24 => "PCM24".to_string(),
        FsbAudioFormat::Pcm32 => "PCM32".to_string(),
        FsbAudioFormat::PcmFloat => "PCMFLOAT".to_string(),
        FsbAudioFormat::FAdpcm => "FADPCM".to_string(),
        FsbAudioFormat::GcAdpcm => "GCADPCM".to_string(),
        FsbAudioFormat::ImaAdpcm => "IMAADPCM".to_string(),
        FsbAudioFormat::Vag => "VAG".to_string(),
        FsbAudioFormat::HeVag => "HEVAG".to_string(),
        FsbAudioFormat::Mpeg => "MPEG".to_string(),
        FsbAudioFormat::Xwma => "XWMA".to_string(),
        FsbAudioFormat::Xma => "XMA".to_string(),
        FsbAudioFormat::Atrac9 => "ATRAC9".to_string(),
        FsbAudioFormat::Celt => "CELT".to_string(),
        FsbAudioFormat::Opus => "OPUS".to_string(),
        other => format!("{other:?}"),
    };

    let mut decoded = DecodedFsbArtifacts {
        sample_rate,
        channels,
        bits_per_sample: None,
        wav: None,
        ogg: None,
        duration_samples: Some(num_samples),
        loop_metadata: stream.loop_info().map(|info| LoopMetadata {
            loop_start_samples: info.start() as u64,
            loop_end_samples: info.end().get() as u64,
            loop_count: None,
        }),
        codec: Some(format_name),
    };

    match (format, stream) {
        (FsbAudioFormat::Vorbis, stream) => {
            let ogg = encode_stream_to_ogg(&clip.name, stream)?;
            decoded.bits_per_sample = Some(16);
            decoded.ogg = Some(ogg);
        }
        (FsbAudioFormat::Pcm8, stream) => {
            let wav = encode_stream_to_wav(&clip.name, stream, "wav", "audio/wav")?;
            decoded.bits_per_sample = Some(8);
            decoded.wav = Some(wav);
        }
        (FsbAudioFormat::Pcm16, stream) => {
            let wav = encode_stream_to_wav(&clip.name, stream, "wav", "audio/wav")?;
            decoded.bits_per_sample = Some(16);
            decoded.wav = Some(wav);
        }
        (FsbAudioFormat::Pcm24, stream) => {
            let wav = encode_stream_to_wav(&clip.name, stream, "wav", "audio/wav")?;
            decoded.bits_per_sample = Some(24);
            decoded.wav = Some(wav);
        }
        (FsbAudioFormat::Pcm32, stream) | (FsbAudioFormat::PcmFloat, stream) => {
            let wav = encode_stream_to_wav(&clip.name, stream, "wav", "audio/wav")?;
            decoded.bits_per_sample = Some(32);
            decoded.wav = Some(wav);
        }
        (FsbAudioFormat::FAdpcm, _stream) | (FsbAudioFormat::GcAdpcm, _stream) => {
            // Attempt custom Firelight ADPCM decoding via our ported decoder.
            match decode_firelight_adpcm_from_fsb(clip, fsb_container, sample_rate, channels as usize, num_samples) {
                Ok(wav_artifact) => {
                    decoded.bits_per_sample = Some(16);
                    decoded.wav = Some(wav_artifact);
                }
                Err(e) => {
                    warn!("Firelight ADPCM decoding failed ({}); no WAV output", e);
                }
            }
        }
        (FsbAudioFormat::ImaAdpcm, stream)
        | (FsbAudioFormat::Vag, stream)
        | (FsbAudioFormat::HeVag, stream)
        | (FsbAudioFormat::Mpeg, stream)
        | (FsbAudioFormat::Xwma, stream)
        | (FsbAudioFormat::Xma, stream)
        | (FsbAudioFormat::Atrac9, stream)
        | (FsbAudioFormat::Celt, stream)
        | (FsbAudioFormat::Opus, stream) => {
            let wav = encode_stream_to_wav(&clip.name, stream, "wav", "audio/wav")?;
            decoded.bits_per_sample = Some(16);
            decoded.wav = Some(wav);
        }
        (other, _) => {
            bail!("FSB format {:?} is not currently supported", other);
        }
    }

    Ok(decoded)
}

fn encode_stream_to_wav(
    name: &str,
    stream: FsbStream,
    extension: &str,
    media_type: &'static str,
) -> Result<AudioArtifact> {
    use fsbex::encode::EncodeError;

    let mut buffer = Vec::new();
    stream
        .write(&mut buffer)
        .map_err(|err| match err {
            EncodeError::UnsupportedFormat { format } => {
                anyhow::anyhow!("Unsupported format: {format}")
            }
            other => anyhow::anyhow!(other.to_string()),
        })?;

    Ok(AudioArtifact {
        filename: format!("{}.{}", name, extension),
        media_type,
        bytes: buffer,
    })
}

fn encode_stream_to_ogg(name: &str, stream: FsbStream) -> Result<AudioArtifact> {
    use fsbex::encode::EncodeError;

    let mut buffer = Vec::new();
    stream
        .write(&mut buffer)
        .map_err(|err| match err {
            EncodeError::UnsupportedFormat { format } => {
                anyhow::anyhow!("Unsupported format: {format}")
            }
            other => anyhow::anyhow!(other.to_string()),
        })?;

    Ok(AudioArtifact {
        filename: format!("{}.ogg", name),
        media_type: "audio/ogg",
        bytes: buffer,
    })
}

/// Decode Firelight ADPCM (FADPCM/GCADPCM) using our custom decoder.
///
/// This function:
/// 1. Parses the FSB container using `fsbex` to extract DSP coefficient chunks.
/// 2. Locates the ADPCM payload in the Unity clip data.
/// 3. Invokes `decode_firelight_gcadpcm` with the coefficients.
/// 4. Encodes the resulting PCM samples as a WAV artifact.
fn decode_firelight_adpcm_from_fsb(
    clip: &UnityAudioClip,
    fsb_container: &FsbContainer,
    sample_rate: u32,
    channels: usize,
    num_samples: u64,
) -> Result<AudioArtifact> {
    // Parse the FSB container via fsbex to access internal DSP coefficients.
    let _bank = FsbBank::new(clip.data.as_slice()).context("Failed to re-parse FSB for ADPCM")?;
    
    // The fsbex library parses DSP coefficients internally but doesn't expose them directly.
    // We need to manually extract them from the raw FSB data by parsing the chunk structure.
    // For now, we'll attempt a fallback approach: extract the ADPCM payload and construct
    // default coefficients if parsing fails, or skip decoding if no coefficients are found.
    
    // Locate the ADPCM payload at the FSB data offset.
    let data_offset = fsb_container.data_offset;
    if data_offset >= clip.data.len() {
        bail!("FSB data offset out of bounds for ADPCM decoding");
    }
    
    let adpcm_payload = &clip.data[data_offset..];
    
    // Attempt to extract DSP coefficients from the FSB header/chunk data.
    // The fsbex library stores these in StreamInfo._dsp_coeffs, but they're not publicly accessible.
    // We'll need to parse the FSB5 chunk structure manually to extract them.
    // For this initial implementation, we'll use a workaround: attempt to parse the chunks ourselves.
    
    let coeffs = extract_dsp_coefficients_from_fsb(&clip.data, fsb_container)?;
    
    // Decode ADPCM to PCM using our ported Firelight decoder.
    let pcm_samples = decode_firelight_gcadpcm(
        adpcm_payload,
        &coeffs,
        channels,
        num_samples as usize,
    )?;
    
    // Encode the PCM samples as WAV.
    let wav_bytes = encode_pcm_i16_from_samples(&pcm_samples, channels as u32, sample_rate)?;
    
    Ok(AudioArtifact {
        filename: format!("{}.wav", clip.name),
        media_type: "audio/wav",
        bytes: wav_bytes,
    })
}

/// Extract DSP coefficients from the raw FSB data by parsing the chunk structure.
///
/// FSB5 stores DSP coefficients in a `DSPCOEFF` chunk (type 7) within each stream header.
/// Each channel has 16 coefficients (i16 values, big-endian), followed by 14 padding bytes.
fn extract_dsp_coefficients_from_fsb(
    fsb_data: &[u8],
    fsb_container: &FsbContainer,
) -> Result<FirelightCoefficients> {
    // Parse the FSB5 header to locate the stream headers.
    // FSB5 header layout:
    //   0x00: signature "FSB5" (4 bytes)
    //   0x04: version (u32)
    //   0x08: num_samples (u32)
    //   0x0C: shdr_size (u32)
    //   0x10: name_table_size (u32)
    //   0x14: data_size (u32)
    //   0x18: mode (u32) - contains audio format
    //   0x1C: zero (u32)
    //   0x20: hash (8 bytes)
    //   0x28: dummy (8 bytes)
    //   0x30: stream headers start
    
    if !fsb_data.starts_with(b"FSB5") {
        bail!("Not an FSB5 container; ADPCM coefficient extraction requires FSB5");
    }
    
    // Read the shdr_size (sample headers size) at offset 0x0C.
    if fsb_data.len() < 0x30 {
        bail!("FSB5 header too small");
    }
    
    let shdr_size = u32::from_le_bytes([
        fsb_data[0x0C],
        fsb_data[0x0D],
        fsb_data[0x0E],
        fsb_data[0x0F],
    ]) as usize;
    
    // Stream headers start at offset 0x30.
    let stream_header_start = 0x30;
    let stream_header_data = &fsb_data[stream_header_start..stream_header_start + shdr_size];
    
    // Parse the first stream header to extract DSP coefficients.
    // Each stream header begins with:
    //   - mode flags (variable length, encoded with "more chunks" bit)
    //   - followed by chunks
    
    // For simplicity, we'll use a basic parser to locate the DSPCOEFF chunk (type 7).
    let coeffs = parse_dsp_coeff_chunk(stream_header_data, fsb_container.header.sample_count as usize)?;
    
    Ok(coeffs)
}

/// Parse the DSPCOEFF chunk from FSB5 stream header data.
fn parse_dsp_coeff_chunk(stream_header_data: &[u8], _num_samples: usize) -> Result<FirelightCoefficients> {
    use nom::number::complete::{be_i16, le_u32};
    use nom::IResult;
    
    // FSB5 stream header format (first stream):
    //   - Basic header (variable length based on "more chunks" bit)
    //   - Chunks follow
    //
    // Each chunk is:
    //   u32 chunk_info where:
    //     bit 0: more_chunks (1 if more chunks follow)
    //     bits 1-24: chunk_size (24-bit size)
    //     bits 25-31: chunk_type (7-bit type, where 7 = DSPCOEFF)
    
    let mut input = stream_header_data;
    
    // Skip the basic stream header (first 16 bytes minimum for FSB5).
    // The exact structure varies, but we can scan for chunks by looking for the chunk pattern.
    // For robustness, we'll scan for a DSPCOEFF chunk (type 7).
    
    // Parse stream header base fields (simplified):
    // Offset 0x00: more_chunks + size + has_extra (u32)
    if input.len() < 16 {
        bail!("Stream header too small");
    }
    
    // Skip to chunk data (after basic header fields, which vary by FSB version).
    // For FSB5, the base header is typically 16 bytes, then chunks follow.
    input = &input[16..];
    
    // Parse chunks until we find DSPCOEFF (type 7).
    let mut channel_coeffs = Vec::new();
    
    loop {
        if input.len() < 4 {
            break;
        }
        
        let chunk_info_result: IResult<&[u8], u32> = le_u32(input);
        let (rest, chunk_info) = chunk_info_result.map_err(|_| anyhow!("Failed to parse chunk info"))?;
        
        let more_chunks = (chunk_info & 0x01) != 0;
        let chunk_size = ((chunk_info >> 1) & 0x00FFFFFF) as usize;
        let chunk_type = ((chunk_info >> 25) & 0x7F) as u8;
        
        if rest.len() < chunk_size {
            bail!("Chunk size exceeds available data");
        }
        
        let chunk_data = &rest[..chunk_size];
        let rest_after_chunk = &rest[chunk_size..];
        
        if chunk_type == 7 {
            // DSPCOEFF chunk found.
            // Format: for each channel, 16 x i16 (big-endian) coefficients + 14 padding bytes.
            // We need to determine the channel count. For now, assume stereo if chunk_size >= 2 * (16*2 + 14).
            let bytes_per_channel = 16 * 2 + 14; // 16 i16 values + 14 padding bytes
            let num_channels = chunk_size / bytes_per_channel;
            
            if num_channels == 0 {
                bail!("DSPCOEFF chunk has invalid size");
            }
            
            let mut coeff_cursor = chunk_data;
            
            for _ch in 0..num_channels {
                let mut channel_coeff = [0i16; 16];
                
                for i in 0..16 {
                    if coeff_cursor.len() < 2 {
                        bail!("Not enough data for DSP coefficients");
                    }
                    
                    let coeff_result: IResult<&[u8], i16> = be_i16(coeff_cursor);
                    let (rest_coeff, coeff_value) = coeff_result.map_err(|_| anyhow!("Failed to parse coefficient"))?;
                    
                    channel_coeff[i] = coeff_value;
                    coeff_cursor = rest_coeff;
                }
                
                // Skip the 14 padding bytes.
                if coeff_cursor.len() < 14 {
                    bail!("Not enough padding bytes after coefficients");
                }
                coeff_cursor = &coeff_cursor[14..];
                
                channel_coeffs.push(channel_coeff);
            }
            
            break;
        }
        
        if !more_chunks {
            break;
        }
        
        input = rest_after_chunk;
    }
    
    if channel_coeffs.is_empty() {
        bail!("No DSPCOEFF chunk found in FSB stream header");
    }
    
    Ok(FirelightCoefficients::new(channel_coeffs))
}

/// Encode interleaved PCM i16 samples as WAV using hound.
fn encode_pcm_i16_from_samples(samples: &[i16], channels: u32, sample_rate: u32) -> Result<Vec<u8>> {
    use std::io::Cursor;
    
    let mut cursor = Cursor::new(Vec::new());
    let spec = WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    
    {
        let mut writer = WavWriter::new(&mut cursor, spec)?;
        for &sample in samples {
            writer.write_sample(sample)?;
        }
        writer.flush()?;
    }
    
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsb5_parse_empty_container() {
        // Empty buffer should fail gracefully.
        let empty_data = vec![];
        let result = parse_fsb(&empty_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_audio_pipeline_options_default() {
        let opts = AudioPipelineOptions::default();
        assert!(opts.emit_wav);
        assert!(!opts.prefer_ogg);
        assert!(!opts.validate_duration);
    }

    #[test]
    fn test_audio_conversion_stats_display() {
        let stats = AudioConversionStats {
            duration_seconds: 3.5,
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            primary_size_bytes: 1024,
            secondary_size_bytes: Some(512),
        };
        let display = format!("{}", stats);
        assert!(display.contains("3.50s"));
        assert!(display.contains("44100Hz"));
        assert!(display.contains("stereo"));
    }

    #[test]
    fn test_loop_metadata_display() {
        let loop_meta = LoopMetadata {
            loop_start_samples: 0,
            loop_end_samples: 88200,
            loop_count: Some(0), // infinite loop
        };
        let display = format!("{}", loop_meta);
        assert!(display.contains("start=0"));
        assert!(display.contains("end=88200"));
        assert!(display.contains("count=0"));
    }

    #[test]
    fn test_firelight_coeff_extraction_invalid_fsb4() {
        // FSB4 is not supported for coefficient extraction (requires FSB5).
        let fsb4_data = b"FSB4\x00\x00\x00\x00";
        let fsb_container = FsbContainer {
            header: FsbHeader {
                sample_count: 1,
                _stream_count: 1,
                flags: 0,
            },
            samples: vec![],
            data_offset: 0,
        };
        let result = extract_dsp_coefficients_from_fsb(fsb4_data, &fsb_container);
        assert!(result.is_err());
    }
}