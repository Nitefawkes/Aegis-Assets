use anyhow::Result;
use thiserror::Error;
use tracing::debug;

/// Security limits for decompression operations
#[derive(Debug, Clone)]
pub struct DecompressionLimits {
    /// Maximum allowed decompressed size in bytes
    pub max_decompressed_size: usize,
    /// Maximum compression ratio (decompressed / compressed)
    pub max_compression_ratio: f64,
    /// Maximum decompression time in milliseconds
    pub timeout_ms: u64,
    /// Memory limit in bytes (None = unlimited)
    pub memory_limit: Option<usize>,
}

impl Default for DecompressionLimits {
    fn default() -> Self {
        Self {
            max_decompressed_size: 512 * 1024 * 1024, // 512MB max
            max_compression_ratio: 1000.0,             // 1000:1 max ratio
            timeout_ms: 30_000,                        // 30 second timeout
            memory_limit: Some(1024 * 1024 * 1024),   // 1GB memory limit
        }
    }
}

impl DecompressionLimits {
    /// Create limits for small files (more restrictive)
    pub fn small_file() -> Self {
        Self {
            max_decompressed_size: 64 * 1024 * 1024, // 64MB max
            max_compression_ratio: 100.0,             // 100:1 max ratio
            timeout_ms: 5_000,                        // 5 second timeout
            memory_limit: Some(128 * 1024 * 1024),   // 128MB memory limit
        }
    }

    /// Create limits for enterprise environments (most restrictive)
    pub fn enterprise() -> Self {
        Self {
            max_decompressed_size: 256 * 1024 * 1024, // 256MB max
            max_compression_ratio: 50.0,               // 50:1 max ratio
            timeout_ms: 10_000,                        // 10 second timeout
            memory_limit: Some(512 * 1024 * 1024),    // 512MB memory limit
        }
    }
}

#[derive(Debug, Error)]
pub enum CompressionError {
    #[error("Decompressed size {requested} exceeds limit {limit}")]
    ExceedsMaxSize { requested: usize, limit: usize },
    
    #[error("Compression ratio {ratio:.2} exceeds limit {limit:.2} (potential bomb)")]
    SuspiciousCompressionRatio { ratio: f64, limit: f64 },
    
    #[error("Size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },
    
    #[error("Decompression timeout exceeded: {limit_ms}ms")]
    TimeoutExceeded { limit_ms: u64 },
    
    #[error("Memory limit exceeded: {used} > {limit}")]
    MemoryLimitExceeded { used: usize, limit: usize },
    
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
    
    #[error("Data corruption detected: {0}")]
    DataCorruption(String),
}

/// Safely decompress LZ4 data with security limits
pub fn decompress_lz4_safe(
    compressed_data: &[u8],
    expected_size: usize,
    limits: &DecompressionLimits,
) -> Result<Vec<u8>, CompressionError> {
    // SECURITY CHECK 1: Validate expected size
    if expected_size > limits.max_decompressed_size {
        return Err(CompressionError::ExceedsMaxSize {
            requested: expected_size,
            limit: limits.max_decompressed_size,
        });
    }

    // SECURITY CHECK 2: Validate compression ratio
    if compressed_data.is_empty() {
        return Err(CompressionError::DataCorruption(
            "Empty compressed data".to_string()
        ));
    }
    
    let compression_ratio = expected_size as f64 / compressed_data.len() as f64;
    if compression_ratio > limits.max_compression_ratio {
        return Err(CompressionError::SuspiciousCompressionRatio {
            ratio: compression_ratio,
            limit: limits.max_compression_ratio,
        });
    }

    // SECURITY CHECK 3: Memory limit check
    if let Some(memory_limit) = limits.memory_limit {
        let total_memory_needed = compressed_data.len() + expected_size;
        if total_memory_needed > memory_limit {
            return Err(CompressionError::MemoryLimitExceeded {
                used: total_memory_needed,
                limit: memory_limit,
            });
        }
    }

    debug!(
        "LZ4 decompression: {} bytes -> {} bytes (ratio: {:.2})",
        compressed_data.len(),
        expected_size,
        compression_ratio
    );

    // SECURITY CHECK 4: Timeout protection
    let start_time = std::time::Instant::now();
    
    // Perform decompression with size validation
    let mut decompressed = vec![0u8; expected_size];
    let actual_size = lz4_flex::decompress_into(compressed_data, &mut decompressed)
        .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;

    // SECURITY CHECK 5: Validate actual decompressed size
    if actual_size != expected_size {
        return Err(CompressionError::SizeMismatch {
            expected: expected_size,
            actual: actual_size,
        });
    }

    // SECURITY CHECK 6: Check timeout
    let elapsed = start_time.elapsed();
    if elapsed.as_millis() > limits.timeout_ms as u128 {
        return Err(CompressionError::TimeoutExceeded {
            limit_ms: limits.timeout_ms,
        });
    }

    debug!("LZ4 decompression completed in {:?}", elapsed);
    
    decompressed.truncate(actual_size);
    Ok(decompressed)
}

/// Safely decompress LZMA data with security limits
pub fn decompress_lzma_safe(
    compressed_data: &[u8],
    expected_size: usize,
    limits: &DecompressionLimits,
) -> Result<Vec<u8>, CompressionError> {
    // SECURITY CHECK 1: Validate expected size
    if expected_size > limits.max_decompressed_size {
        return Err(CompressionError::ExceedsMaxSize {
            requested: expected_size,
            limit: limits.max_decompressed_size,
        });
    }

    // SECURITY CHECK 2: Basic format validation
    if compressed_data.len() < 13 {
        return Err(CompressionError::DataCorruption(
            "LZMA data too short (missing header)".to_string()
        ));
    }

    // Parse Unity LZMA format
    let uncompressed_size = u64::from_le_bytes([
        compressed_data[5], compressed_data[6], compressed_data[7], compressed_data[8],
        compressed_data[9], compressed_data[10], compressed_data[11], compressed_data[12],
    ]);

    // SECURITY CHECK 3: Validate header size against expected
    let size_tolerance = expected_size / 10; // Allow 10% tolerance
    if (uncompressed_size as usize).abs_diff(expected_size) > size_tolerance {
        return Err(CompressionError::SizeMismatch {
            expected: expected_size,
            actual: uncompressed_size as usize,
        });
    }

    // SECURITY CHECK 4: Validate compression ratio
    let lzma_data_size = compressed_data.len() - 13;
    if lzma_data_size == 0 {
        return Err(CompressionError::DataCorruption(
            "No LZMA data after header".to_string()
        ));
    }
    
    let compression_ratio = uncompressed_size as f64 / lzma_data_size as f64;
    if compression_ratio > limits.max_compression_ratio {
        return Err(CompressionError::SuspiciousCompressionRatio {
            ratio: compression_ratio,
            limit: limits.max_compression_ratio,
        });
    }

    // SECURITY CHECK 5: Memory limit check
    if let Some(memory_limit) = limits.memory_limit {
        let total_memory_needed = compressed_data.len() + uncompressed_size as usize;
        if total_memory_needed > memory_limit {
            return Err(CompressionError::MemoryLimitExceeded {
                used: total_memory_needed,
                limit: memory_limit,
            });
        }
    }

    debug!(
        "LZMA decompression: {} bytes -> {} bytes (ratio: {:.2})",
        lzma_data_size,
        uncompressed_size,
        compression_ratio
    );

    // SECURITY CHECK 6: Timeout protection
    let start_time = std::time::Instant::now();

    // Perform LZMA decompression
    let lzma_data = &compressed_data[13..];
    let mut output = Vec::new();
    let mut cursor = std::io::Cursor::new(lzma_data);
    
    lzma_rs::lzma_decompress_with_options(
        &mut cursor,
        &mut output,
        &lzma_rs::decompress::Options {
            unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(uncompressed_size)),
            memlimit: limits.memory_limit,
            allow_incomplete: false,
        },
    ).map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;

    // SECURITY CHECK 7: Validate output size
    if output.len() != expected_size {
        return Err(CompressionError::SizeMismatch {
            expected: expected_size,
            actual: output.len(),
        });
    }

    // SECURITY CHECK 8: Check timeout
    let elapsed = start_time.elapsed();
    if elapsed.as_millis() > limits.timeout_ms as u128 {
        return Err(CompressionError::TimeoutExceeded {
            limit_ms: limits.timeout_ms,
        });
    }

    debug!("LZMA decompression completed in {:?}", elapsed);
    Ok(output)
}

/// Decompress data using the appropriate algorithm with security limits
pub fn decompress_safe(
    compressed_data: &[u8],
    expected_size: usize,
    compression_type: CompressionType,
    limits: &DecompressionLimits,
) -> Result<Vec<u8>, CompressionError> {
    match compression_type {
        CompressionType::Lz4 => decompress_lz4_safe(compressed_data, expected_size, limits),
        CompressionType::Lzma => decompress_lzma_safe(compressed_data, expected_size, limits),
        CompressionType::None => {
            // Validate uncompressed data size
            if compressed_data.len() != expected_size {
                return Err(CompressionError::SizeMismatch {
                    expected: expected_size,
                    actual: compressed_data.len(),
                });
            }
            Ok(compressed_data.to_vec())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    None,
    Lz4,
    Lzma,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompression_size_limit() {
        let limits = DecompressionLimits {
            max_decompressed_size: 100,
            ..Default::default()
        };

        let dummy_data = vec![0u8; 10];
        let result = decompress_lz4_safe(&dummy_data, 200, &limits);
        
        match result {
            Err(CompressionError::ExceedsMaxSize { requested, limit }) => {
                assert_eq!(requested, 200);
                assert_eq!(limit, 100);
            }
            _ => panic!("Expected ExceedsMaxSize error"),
        }
    }

    #[test]
    fn test_compression_ratio_limit() {
        let limits = DecompressionLimits {
            max_compression_ratio: 10.0,
            ..Default::default()
        };

        let small_data = vec![0u8; 1];  // 1 byte compressed
        let result = decompress_lz4_safe(&small_data, 100, &limits); // 100 bytes decompressed = 100:1 ratio
        
        match result {
            Err(CompressionError::SuspiciousCompressionRatio { ratio, limit }) => {
                assert_eq!(ratio, 100.0);
                assert_eq!(limit, 10.0);
            }
            _ => panic!("Expected SuspiciousCompressionRatio error"),
        }
    }

    #[test]
    fn test_empty_data_protection() {
        let limits = DecompressionLimits::default();
        let empty_data = vec![];
        
        let result = decompress_lz4_safe(&empty_data, 100, &limits);
        match result {
            Err(CompressionError::DataCorruption(_)) => {
                // Expected
            }
            _ => panic!("Expected DataCorruption error for empty data"),
        }
    }
}