use anyhow::{Result, Context, bail};
use std::io::{Read, BufRead, BufReader, Cursor};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, warn, info};

/// Streaming decompression statistics
#[derive(Debug, Clone)]
pub struct StreamingStats {
    pub bytes_read: usize,
    pub bytes_written: usize,
    pub chunks_processed: usize,
    pub peak_memory_usage: usize,
    pub compression_ratio: f64,
}

/// Memory pressure monitor for streaming operations
pub struct MemoryPressureMonitor {
    current_usage: Arc<AtomicUsize>,
    peak_usage: Arc<AtomicUsize>,
    memory_limit: usize,
}

impl MemoryPressureMonitor {
    pub fn new(memory_limit_mb: usize) -> Self {
        Self {
            current_usage: Arc::new(AtomicUsize::new(0)),
            peak_usage: Arc::new(AtomicUsize::new(0)),
            memory_limit: memory_limit_mb * 1024 * 1024,
        }
    }

    pub fn allocate(&self, size: usize) -> Result<()> {
        let current = self.current_usage.fetch_add(size, Ordering::SeqCst) + size;
        
        // Update peak usage
        let current_peak = self.peak_usage.load(Ordering::SeqCst);
        if current > current_peak {
            self.peak_usage.store(current, Ordering::SeqCst);
        }

        // Check memory pressure
        if current > self.memory_limit {
            self.current_usage.fetch_sub(size, Ordering::SeqCst);
            bail!("Memory limit exceeded: {}MB > {}MB", 
                  current / 1024 / 1024, 
                  self.memory_limit / 1024 / 1024);
        }

        Ok(())
    }

    pub fn deallocate(&self, size: usize) {
        self.current_usage.fetch_sub(size, Ordering::SeqCst);
    }

    pub fn current_usage_mb(&self) -> f64 {
        self.current_usage.load(Ordering::SeqCst) as f64 / 1024.0 / 1024.0
    }

    pub fn peak_usage_mb(&self) -> f64 {
        self.peak_usage.load(Ordering::SeqCst) as f64 / 1024.0 / 1024.0
    }

    pub fn memory_pressure(&self) -> f64 {
        self.current_usage.load(Ordering::SeqCst) as f64 / self.memory_limit as f64
    }
}

/// Streaming buffer with automatic memory management
pub struct StreamingBuffer {
    data: Vec<u8>,
    capacity: usize,
    read_pos: usize,
    write_pos: usize,
    monitor: Arc<MemoryPressureMonitor>,
    allocated_size: usize,
}

impl StreamingBuffer {
    pub fn new(initial_capacity: usize, monitor: Arc<MemoryPressureMonitor>) -> Result<Self> {
        monitor.allocate(initial_capacity)?;
        
        Ok(Self {
            data: Vec::with_capacity(initial_capacity),
            capacity: initial_capacity,
            read_pos: 0,
            write_pos: 0,
            monitor,
            allocated_size: initial_capacity,
        })
    }

    pub fn available_capacity(&self) -> usize {
        self.capacity - self.write_pos
    }

    pub fn available_data(&self) -> usize {
        self.write_pos - self.read_pos
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        // Resize buffer if needed
        let required_capacity = self.write_pos + data.len();
        if required_capacity > self.capacity {
            self.resize(required_capacity)?;
        }

        // Ensure vector has enough capacity
        if self.data.len() < required_capacity {
            self.data.resize(required_capacity, 0);
        }

        // Copy data
        let copy_len = data.len();
        self.data[self.write_pos..self.write_pos + copy_len].copy_from_slice(data);
        self.write_pos += copy_len;

        Ok(copy_len)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        if buf.is_empty() || self.read_pos >= self.write_pos {
            return 0;
        }

        let available = self.write_pos - self.read_pos;
        let to_read = buf.len().min(available);
        
        buf[..to_read].copy_from_slice(&self.data[self.read_pos..self.read_pos + to_read]);
        self.read_pos += to_read;

        to_read
    }

    pub fn peek(&self, buf: &mut [u8]) -> usize {
        if buf.is_empty() || self.read_pos >= self.write_pos {
            return 0;
        }

        let available = self.write_pos - self.read_pos;
        let to_read = buf.len().min(available);
        
        buf[..to_read].copy_from_slice(&self.data[self.read_pos..self.read_pos + to_read]);

        to_read
    }

    pub fn consume(&mut self, amount: usize) {
        self.read_pos = (self.read_pos + amount).min(self.write_pos);
        
        // Compact buffer if read position is more than half the capacity
        if self.read_pos > self.capacity / 2 {
            self.compact();
        }
    }

    fn resize(&mut self, new_capacity: usize) -> Result<()> {
        let additional_memory = new_capacity - self.allocated_size;
        self.monitor.allocate(additional_memory)?;

        self.capacity = new_capacity;
        self.data.reserve(new_capacity - self.data.len());
        self.allocated_size = new_capacity;

        Ok(())
    }

    fn compact(&mut self) {
        if self.read_pos == 0 {
            return;
        }

        // Move unread data to the beginning
        let unread_data = self.write_pos - self.read_pos;
        if unread_data > 0 {
            self.data.copy_within(self.read_pos..self.write_pos, 0);
        }

        self.write_pos = unread_data;
        self.read_pos = 0;
    }

    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }
}

impl Drop for StreamingBuffer {
    fn drop(&mut self) {
        self.monitor.deallocate(self.allocated_size);
    }
}

/// Streaming LZ4 decompressor
pub struct StreamingLZ4Decompressor {
    buffer: StreamingBuffer,
    monitor: Arc<MemoryPressureMonitor>,
    stats: StreamingStats,
}

impl StreamingLZ4Decompressor {
    pub fn new(monitor: Arc<MemoryPressureMonitor>) -> Result<Self> {
        const INITIAL_BUFFER_SIZE: usize = 64 * 1024; // 64KB
        
        let buffer = StreamingBuffer::new(INITIAL_BUFFER_SIZE, monitor.clone())?;
        
        Ok(Self {
            buffer,
            monitor,
            stats: StreamingStats {
                bytes_read: 0,
                bytes_written: 0,
                chunks_processed: 0,
                peak_memory_usage: 0,
                compression_ratio: 0.0,
            },
        })
    }

    pub fn decompress_chunk(&mut self, compressed_data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        debug!("Decompressing LZ4 chunk: {} bytes -> {} bytes expected", 
               compressed_data.len(), expected_size);

        // Check memory pressure before allocating
        if self.monitor.memory_pressure() > 0.8 {
            warn!("High memory pressure ({:.1}%), consider reducing chunk size", 
                  self.monitor.memory_pressure() * 100.0);
        }

        // Use lz4_flex for decompression
        let decompressed = lz4_flex::decompress(compressed_data, expected_size)
            .context("Failed to decompress LZ4 data")?;

        // Update statistics
        self.stats.bytes_read += compressed_data.len();
        self.stats.bytes_written += decompressed.len();
        self.stats.chunks_processed += 1;
        self.stats.peak_memory_usage = self.stats.peak_memory_usage.max(
            self.monitor.current_usage_mb() as usize
        );
        self.stats.compression_ratio = if self.stats.bytes_read > 0 {
            1.0 - (self.stats.bytes_written as f64 / self.stats.bytes_read as f64)
        } else {
            0.0
        };

        Ok(decompressed)
    }

    pub fn stats(&self) -> &StreamingStats {
        &self.stats
    }
}

/// Streaming LZMA decompressor
pub struct StreamingLZMADecompressor {
    buffer: StreamingBuffer,
    monitor: Arc<MemoryPressureMonitor>,
    stats: StreamingStats,
}

impl StreamingLZMADecompressor {
    pub fn new(monitor: Arc<MemoryPressureMonitor>) -> Result<Self> {
        const INITIAL_BUFFER_SIZE: usize = 128 * 1024; // 128KB for LZMA
        
        let buffer = StreamingBuffer::new(INITIAL_BUFFER_SIZE, monitor.clone())?;
        
        Ok(Self {
            buffer,
            monitor,
            stats: StreamingStats {
                bytes_read: 0,
                bytes_written: 0,
                chunks_processed: 0,
                peak_memory_usage: 0,
                compression_ratio: 0.0,
            },
        })
    }

    pub fn decompress_chunk(&mut self, compressed_data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        debug!("Decompressing LZMA chunk: {} bytes -> {} bytes expected", 
               compressed_data.len(), expected_size);

        if compressed_data.len() < 13 {
            bail!("LZMA data too short (missing header)");
        }

        // Check memory pressure
        if self.monitor.memory_pressure() > 0.9 {
            warn!("Critical memory pressure ({:.1}%), may fail allocation", 
                  self.monitor.memory_pressure() * 100.0);
        }

        // Extract LZMA parameters (Unity format)
        let _properties = &compressed_data[0..5];
        let uncompressed_size = u64::from_le_bytes([
            compressed_data[5], compressed_data[6], compressed_data[7], compressed_data[8],
            compressed_data[9], compressed_data[10], compressed_data[11], compressed_data[12],
        ]);

        // Validate size
        if uncompressed_size as usize > expected_size * 10 {
            bail!("LZMA size mismatch: expected ~{}, got {} (likely corrupted)", 
                  expected_size, uncompressed_size);
        }

        let lzma_data = &compressed_data[13..];

        // Decompress using lzma-rs
        let mut output = Vec::new();
        let mut cursor = Cursor::new(lzma_data);
        
        lzma_rs::lzma_decompress_with_options(
            &mut cursor,
            &mut output,
            &lzma_rs::decompress::Options {
                unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(uncompressed_size)),
                memlimit: Some(self.monitor.memory_limit),
                allow_incomplete: false,
            },
        ).context("Failed to decompress LZMA data")?;

        // Update statistics
        self.stats.bytes_read += compressed_data.len();
        self.stats.bytes_written += output.len();
        self.stats.chunks_processed += 1;
        self.stats.peak_memory_usage = self.stats.peak_memory_usage.max(
            self.monitor.current_usage_mb() as usize
        );
        self.stats.compression_ratio = if self.stats.bytes_read > 0 {
            1.0 - (self.stats.bytes_written as f64 / self.stats.bytes_read as f64)
        } else {
            0.0
        };

        Ok(output)
    }

    pub fn stats(&self) -> &StreamingStats {
        &self.stats
    }
}

/// Placeholder streaming LZHAM decompressor
pub struct StreamingLZHAMDecompressor {
    _buffer: StreamingBuffer,
    monitor: Arc<MemoryPressureMonitor>,
    stats: StreamingStats,
}

impl StreamingLZHAMDecompressor {
    pub fn new(monitor: Arc<MemoryPressureMonitor>) -> Result<Self> {
        const INITIAL_BUFFER_SIZE: usize = 256 * 1024; // 256KB for LZHAM
        
        let buffer = StreamingBuffer::new(INITIAL_BUFFER_SIZE, monitor.clone())?;
        
        Ok(Self {
            _buffer: buffer,
            monitor,
            stats: StreamingStats {
                bytes_read: 0,
                bytes_written: 0,
                chunks_processed: 0,
                peak_memory_usage: 0,
                compression_ratio: 0.0,
            },
        })
    }

    pub fn decompress_chunk(&mut self, _compressed_data: &[u8], _expected_size: usize) -> Result<Vec<u8>> {
        // TODO: Implement LZHAM decompression
        // For now, return an error to maintain the existing behavior
        bail!("LZHAM streaming decompression not yet implemented")
    }

    pub fn stats(&self) -> &StreamingStats {
        &self.stats
    }
}

/// Unified streaming decompressor that handles all Unity compression types
pub struct UnityStreamingDecompressor {
    lz4: StreamingLZ4Decompressor,
    lzma: StreamingLZMADecompressor,
    lzham: StreamingLZHAMDecompressor,
    monitor: Arc<MemoryPressureMonitor>,
}

impl UnityStreamingDecompressor {
    pub fn new(memory_limit_mb: usize) -> Result<Self> {
        let monitor = Arc::new(MemoryPressureMonitor::new(memory_limit_mb));
        
        let lz4 = StreamingLZ4Decompressor::new(monitor.clone())?;
        let lzma = StreamingLZMADecompressor::new(monitor.clone())?;
        let lzham = StreamingLZHAMDecompressor::new(monitor.clone())?;

        Ok(Self {
            lz4,
            lzma,
            lzham,
            monitor,
        })
    }

    pub fn decompress_chunk(&mut self, compressed_data: &[u8], compression_type: u32, expected_size: usize) -> Result<Vec<u8>> {
        debug!("Streaming decompress: type={}, input={} bytes, expected={} bytes", 
               compression_type, compressed_data.len(), expected_size);

        match compression_type {
            0 => {
                // No compression
                if compressed_data.len() != expected_size {
                    bail!("Uncompressed size mismatch: expected {}, got {}", 
                          expected_size, compressed_data.len());
                }
                Ok(compressed_data.to_vec())
            }
            1 => {
                // LZMA
                self.lzma.decompress_chunk(compressed_data, expected_size)
            }
            2 | 3 => {
                // LZ4 and LZ4HC
                self.lz4.decompress_chunk(compressed_data, expected_size)
            }
            4 => {
                // LZHAM
                self.lzham.decompress_chunk(compressed_data, expected_size)
            }
            _ => {
                warn!("Unknown compression type: {} - attempting fallback", compression_type);
                
                if compressed_data.len() == expected_size {
                    warn!("Compression type {} unknown, but data size matches expected size - returning raw data", compression_type);
                    Ok(compressed_data.to_vec())
                } else {
                    warn!("Compression type {} unknown - data size mismatch ({} vs expected {}) - returning empty",
                          compression_type, compressed_data.len(), expected_size);
                    Ok(Vec::new())
                }
            }
        }
    }

    pub fn memory_stats(&self) -> (f64, f64) {
        (self.monitor.current_usage_mb(), self.monitor.peak_usage_mb())
    }

    pub fn compression_stats(&self) -> (StreamingStats, StreamingStats) {
        (self.lz4.stats().clone(), self.lzma.stats().clone())
    }

    pub fn memory_pressure(&self) -> f64 {
        self.monitor.memory_pressure()
    }
}

/// Chunked file reader for large Unity files
pub struct ChunkedUnityReader<R: Read> {
    inner: BufReader<R>,
    chunk_size: usize,
    total_read: u64,
    file_size: Option<u64>,
}

impl<R: Read> ChunkedUnityReader<R> {
    pub fn new(reader: R, chunk_size: usize) -> Self {
        Self {
            inner: BufReader::with_capacity(chunk_size * 2, reader),
            chunk_size,
            total_read: 0,
            file_size: None,
        }
    }

    pub fn with_file_size(mut self, file_size: u64) -> Self {
        self.file_size = Some(file_size);
        self
    }

    pub fn read_chunk(&mut self) -> Result<Option<Vec<u8>>> {
        let mut buffer = vec![0u8; self.chunk_size];
        let bytes_read = self.inner.read(&mut buffer)?;
        
        if bytes_read == 0 {
            return Ok(None);
        }
        
        buffer.truncate(bytes_read);
        self.total_read += bytes_read as u64;
        
        Ok(Some(buffer))
    }

    pub fn progress(&self) -> Option<f64> {
        self.file_size.map(|size| {
            if size == 0 {
                1.0
            } else {
                (self.total_read as f64) / (size as f64)
            }
        })
    }

    pub fn bytes_read(&self) -> u64 {
        self.total_read
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_memory_pressure_monitor() {
        let monitor = MemoryPressureMonitor::new(10); // 10MB limit
        
        // Should succeed
        assert!(monitor.allocate(1024 * 1024).is_ok()); // 1MB
        assert_eq!(monitor.current_usage_mb(), 1.0);
        
        // Should fail
        assert!(monitor.allocate(15 * 1024 * 1024).is_err()); // 15MB
        
        monitor.deallocate(1024 * 1024);
        assert_eq!(monitor.current_usage_mb(), 0.0);
    }

    #[test]
    fn test_streaming_buffer() {
        let monitor = Arc::new(MemoryPressureMonitor::new(100));
        let mut buffer = StreamingBuffer::new(1024, monitor).unwrap();
        
        // Test write
        let data = b"Hello, World!";
        assert_eq!(buffer.write(data).unwrap(), data.len());
        assert_eq!(buffer.available_data(), data.len());
        
        // Test read
        let mut read_buf = vec![0u8; data.len()];
        assert_eq!(buffer.read(&mut read_buf), data.len());
        assert_eq!(&read_buf, data);
        assert_eq!(buffer.available_data(), 0);
    }

    #[test]
    fn test_chunked_reader() {
        let data = b"This is a test file with multiple chunks of data that should be read in smaller pieces.";
        let cursor = Cursor::new(data);
        let mut reader = ChunkedUnityReader::new(cursor, 20);
        
        let mut chunks = Vec::new();
        while let Some(chunk) = reader.read_chunk().unwrap() {
            chunks.push(chunk);
        }
        
        // Verify we got multiple chunks
        assert!(chunks.len() > 1);
        
        // Verify data integrity
        let reconstructed: Vec<u8> = chunks.into_iter().flatten().collect();
        assert_eq!(&reconstructed, data);
    }

    #[test]
    fn test_lz4_streaming_decompressor() {
        let monitor = Arc::new(MemoryPressureMonitor::new(100));
        let mut decompressor = StreamingLZ4Decompressor::new(monitor).unwrap();
        
        let original = b"This is a test string for LZ4 compression. It should compress reasonably well.";
        let compressed = lz4_flex::compress(original);
        
        let decompressed = decompressor.decompress_chunk(&compressed, original.len()).unwrap();
        assert_eq!(&decompressed, original);
        
        let stats = decompressor.stats();
        assert!(stats.bytes_read > 0);
        assert!(stats.bytes_written > 0);
        assert_eq!(stats.chunks_processed, 1);
    }
}
