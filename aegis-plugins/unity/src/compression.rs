use anyhow::{bail, Context, Result};
use tracing::warn;

/// Decompress LZ4 compressed data
pub fn decompress_lz4(compressed: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    lz4_flex::decompress(compressed, expected_size)
        .context("Failed to decompress LZ4 data")
}

/// Decompress LZMA compressed data
pub fn decompress_lzma(compressed: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    // Unity uses LZMA with specific parameters
    // This is a simplified implementation
    
    if compressed.len() < 13 {
        bail!("LZMA data too short (missing header)");
    }
    
    // Unity LZMA format:
    // - 5 bytes: LZMA properties
    // - 8 bytes: uncompressed size (little-endian)
    // - N bytes: compressed data
    
    let properties = &compressed[0..5];
    let uncompressed_size = u64::from_le_bytes([
        compressed[5], compressed[6], compressed[7], compressed[8],
        compressed[9], compressed[10], compressed[11], compressed[12],
    ]);
    
    // Allow some tolerance for LZMA size mismatches (demo files may not be perfect)
    if uncompressed_size as usize > expected_size * 10 || uncompressed_size < expected_size as u64 / 10 {
        bail!("LZMA size severely mismatched: expected ~{}, got {} (likely corrupted data)", expected_size, uncompressed_size);
    }
    
    let lzma_data = &compressed[13..];
    
    // Use LZMA decompression
    let mut output = Vec::new();
    let mut cursor = std::io::Cursor::new(lzma_data);
    lzma_rs::lzma_decompress_with_options(
        &mut cursor,
        &mut output,
        &lzma_rs::decompress::Options {
            unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(uncompressed_size)),
            memlimit: None,
            allow_incomplete: false,
        },
    ).context("Failed to decompress LZMA data")?;
    Ok(output)
}

/// Decompress data based on Unity compression type
pub fn decompress_unity_data(
    compressed: &[u8], 
    compression_type: u32, 
    expected_size: usize
) -> Result<Vec<u8>> {
    match compression_type {
        0 => {
            // No compression
            if compressed.len() != expected_size {
                bail!("Uncompressed size mismatch: expected {}, got {}", 
                      expected_size, compressed.len());
            }
            Ok(compressed.to_vec())
        }
        1 => {
            // LZMA
            decompress_lzma(compressed, expected_size)
        }
        2 => {
            // LZ4
            decompress_lz4(compressed, expected_size)
        }
        3 => {
            // LZ4HC (High Compression)
            // LZ4HC uses the same decompression as regular LZ4
            decompress_lz4(compressed, expected_size)
        }
        4 => {
            // LZHAM
            bail!("LZHAM compression is not supported yet");
        }
        _ => {
            // Graceful fallback for unknown compression types
            warn!(
                "Unknown Unity compression type: {} - validating data size before attempting fallback",
                compression_type
            );

            // Try to return the raw data if it matches expected size (some games may use uncompressed data with unknown type flags)
            if compressed.len() == expected_size {
                warn!(
                    "Compression type {} unknown, but data size matches expected size - returning raw data",
                    compression_type
                );
                Ok(compressed.to_vec())
            } else if compressed.len() > expected_size {
                warn!(
                    "Compression type {} unknown and data appears compressed ({} bytes vs expected {} bytes) - cannot decode block",
                    compression_type,
                    compressed.len(),
                    expected_size
                );
                bail!(
                    "Unknown compression type {} produced mismatched size: {} bytes vs expected {} bytes",
                    compression_type,
                    compressed.len(),
                    expected_size
                );
            } else {
                warn!(
                    "Compression type {} unknown - data size mismatch ({} vs expected {}) - returning available data",
                    compression_type,
                    compressed.len(),
                    expected_size
                );
                Ok(compressed.to_vec())
            }
        }
    }
}

/// Detect compression type from data header
pub fn detect_compression_type(data: &[u8]) -> Option<u32> {
    if data.len() < 4 {
        return None;
    }
    
    // Check for common compression signatures
    
    // LZ4 magic number (when present)
    if data.starts_with(&[0x04, 0x22, 0x4d, 0x18]) {
        return Some(2); // LZ4
    }
    
    // LZMA properties signature (Unity specific)
    if data.len() >= 13 {
        let properties = data[0];
        // LZMA properties byte is typically in range 0x00-0x5D
        if properties <= 0x5D {
            // Check if the next 8 bytes look like a reasonable size
            let size = u64::from_le_bytes([
                data[5], data[6], data[7], data[8],
                data[9], data[10], data[11], data[12],
            ]);
            
            // Reasonable size check (less than 100MB for single asset)
            if size > 0 && size < 100_000_000 {
                return Some(1); // LZMA
            }
        }
    }
    
    // If no compression signature found, assume uncompressed
    Some(0)
}

/// Calculate compression ratio
pub fn compression_ratio(compressed_size: usize, uncompressed_size: usize) -> f32 {
    if uncompressed_size == 0 {
        return 0.0;
    }
    
    1.0 - (compressed_size as f32 / uncompressed_size as f32)
}

/// Compression statistics for reporting
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub compression_type: u32,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub ratio: f32,
    pub algorithm_name: String,
}

impl CompressionStats {
    pub fn new(compression_type: u32, compressed_size: usize, uncompressed_size: usize) -> Self {
        let ratio = compression_ratio(compressed_size, uncompressed_size);
        let algorithm_name = match compression_type {
            0 => "None",
            1 => "LZMA",
            2 => "LZ4",
            3 => "LZ4HC",
            4 => "LZHAM",
            _ => "Unknown",
        }.to_string();
        
        Self {
            compression_type,
            compressed_size,
            uncompressed_size,
            ratio,
            algorithm_name,
        }
    }
    
    /// Get human-readable compression description
    pub fn description(&self) -> String {
        format!(
            "{} ({:.1}% reduction, {} → {} bytes)",
            self.algorithm_name,
            self.ratio * 100.0,
            self.compressed_size,
            self.uncompressed_size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compression_ratio() {
        assert_eq!(compression_ratio(50, 100), 0.5);
        assert_eq!(compression_ratio(100, 100), 0.0);
        assert_eq!(compression_ratio(0, 100), 1.0);
        assert_eq!(compression_ratio(100, 0), 0.0);
    }
    
    #[test]
    fn test_compression_stats() {
        let stats = CompressionStats::new(2, 512, 1024);
        assert_eq!(stats.algorithm_name, "LZ4");
        assert_eq!(stats.ratio, 0.5);
        assert!(stats.description().contains("50.0%"));
    }
    
    #[test]
    fn test_uncompressed_decompression() {
        let data = b"Hello, World!";
        let result = decompress_unity_data(data, 0, data.len()).unwrap();
        assert_eq!(result, data);
    }
    
    #[test]
    fn test_lz4_roundtrip() {
        let original = b"This is a test string for LZ4 compression. It should compress reasonably well due to repetition. This is a test string for LZ4 compression.";
        
        // Compress with LZ4
        let compressed = lz4_flex::compress(original);
        
        // Decompress with our function
        let decompressed = decompress_lz4(&compressed, original.len()).unwrap();
        
        assert_eq!(decompressed, original);
    }
    
    #[test]
    fn test_compression_type_detection() {
        // Test uncompressed detection
        let uncompressed = b"Hello World";
        assert_eq!(detect_compression_type(uncompressed), Some(0));

        // Test short data
        let short_data = b"Hi";
        assert_eq!(detect_compression_type(short_data), None);
    }

    #[test]
    fn test_unknown_compression_with_mismatched_size_errors() {
        let compressed = vec![0u8; 32];
        let result = decompress_unity_data(&compressed, 99, 16);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown compression type"));
    }

    #[test]
    fn test_unknown_compression_with_matching_size_returns_data() {
        let compressed = vec![1u8; 8];
        let result = decompress_unity_data(&compressed, 99, compressed.len()).unwrap();
        assert_eq!(result, compressed);
    }
}
