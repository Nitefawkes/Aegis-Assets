//! Firelight (FMOD) ADPCM decoding helpers, ported from the Fmod5Sharp project.
//!
//! This module currently focuses on the GCADPCM/FADPCM variant used by Unity
//! FSB containers. The implementation is intentionally self-contained so that
//! additional codec flavours (HEVAG, VAG, etc.) can be layered on in future
//! iterations without leaking the details into the main pipeline logic.

use anyhow::Result;

/// Number of bytes consumed per ADPCM frame in Firelight's GCADPCM encoding.
const BYTES_PER_FRAME: usize = 8;
/// Number of decoded PCM samples emitted per frame.
const SAMPLES_PER_FRAME: usize = 14;
/// Number of 4-bit nibbles contained in a frame.
const NIBBLES_PER_FRAME: usize = 16;

/// Signed nibble lookup used by the Firelight ADPCM predictor.
///
/// The values map a 4-bit unsigned nibble to the signed representation specified
/// by the codec (0..7 => 0..7, 8..15 => -8..-1).
const SIGNED_NIBBLES: [i8; 16] = [
    0, 1, 2, 3, 4, 5, 6, 7, -8, -7, -6, -5, -4, -3, -2, -1,
];

/// Firelight encodes a table of 8 coefficient pairs (16 entries) per channel.
#[derive(Debug, Clone)]
pub struct FirelightCoefficients {
    /// Coefficient pairs per channel (`coefficients[channel][pair_index]`).
    pub coefficients: Vec<[i16; 16]>,
}

impl FirelightCoefficients {
    pub fn new(coefficients: Vec<[i16; 16]>) -> Self {
        Self { coefficients }
    }

    pub fn channels(&self) -> usize {
        self.coefficients.len()
    }
}

/// Decode a Firelight GCADPCM/FADPCM payload to 16-bit interleaved PCM.
///
/// * `adpcm` – raw ADPCM payload (interleaved across channels).
/// * `coeffs` – predictor coefficients for each channel.
/// * `channel_count` – number of channels (1 or 2).
/// * `expected_samples` – total samples per channel.
pub fn decode_firelight_gcadpcm(
    adpcm: &[u8],
    coeffs: &FirelightCoefficients,
    channel_count: usize,
    expected_samples: usize,
) -> Result<Vec<i16>> {
    assert_eq!(coeffs.channels(), channel_count);

    let samples = byte_count_to_sample_count(adpcm.len(), channel_count);
    let total_samples_per_channel = expected_samples.max(samples);
    let mut pcm = Vec::with_capacity(total_samples_per_channel * channel_count);

    // History values are per channel.
    let mut hist1 = vec![0i16; channel_count];
    let mut hist2 = vec![0i16; channel_count];

    let mut in_index = 0;
    let mut current_sample = 0usize;

    while current_sample < total_samples_per_channel && in_index < adpcm.len() {
        for channel in 0..channel_count {
            if current_sample >= total_samples_per_channel || in_index >= adpcm.len() {
                break;
            }

            // The combined byte stores predictor (high nibble) and scale (low nibble).
            let predictor_scale = adpcm[in_index];
            in_index += 1;

            let scale = 1 << (predictor_scale as usize & 0x0F);
            let predictor_index = (predictor_scale >> 4) as usize;

            let coeff_table = coeffs.coefficients[channel];
            let coeff1 = coeff_table[predictor_index * 2] as i32;
            let coeff2 = coeff_table[predictor_index * 2 + 1] as i32;

            let samples_remaining = total_samples_per_channel - current_sample;
            let samples_to_read = samples_remaining.min(SAMPLES_PER_FRAME);

            for sample_idx in 0..samples_to_read {
                if in_index >= adpcm.len() {
                    break;
                }

                let nibble = if sample_idx % 2 == 0 {
                    // High nibble of current byte.
                    get_high_nibble_signed(adpcm[in_index])
                } else {
                    let value = get_low_nibble_signed(adpcm[in_index]);
                    in_index += 1;
                    value
                };

                let mut sample = (nibble as i32 * scale) << 11;
                sample += 1024 + (coeff1 * hist1[channel] as i32) + (coeff2 * hist2[channel] as i32);
                sample >>= 11;

                let sample = clamp_i16(sample);

                hist2[channel] = hist1[channel];
                hist1[channel] = sample;

                pcm.push(sample);
                current_sample += 1;

                if current_sample >= total_samples_per_channel {
                    break;
                }
            }

            // If we consumed an odd number of nibbles, advance the byte pointer.
            if samples_to_read % 2 != 0 {
                in_index += 1;
            }
        }
    }

    Ok(pcm)
}

fn get_high_nibble_signed(value: u8) -> i8 {
    SIGNED_NIBBLES[((value >> 4) & 0x0F) as usize]
}

fn get_low_nibble_signed(value: u8) -> i8 {
    SIGNED_NIBBLES[(value & 0x0F) as usize]
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn nibble_count_to_sample_count(nibbles: usize) -> usize {
    let frames = nibbles / NIBBLES_PER_FRAME;
    let extra_nibbles = nibbles % NIBBLES_PER_FRAME;
    let extra_samples = if extra_nibbles < 2 { 0 } else { extra_nibbles - 2 };

    SAMPLES_PER_FRAME * frames + extra_samples
}

fn byte_count_to_sample_count(byte_count: usize, channels: usize) -> usize {
    let nibbles = byte_count * 2;
    nibble_count_to_sample_count(nibbles) / channels.max(1)
}


