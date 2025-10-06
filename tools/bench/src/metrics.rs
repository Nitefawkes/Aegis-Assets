use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub start_time: Instant,
    pub duration: Duration,
    pub file_size_bytes: u64,
    pub peak_memory_mb: f64,
    pub throughput_mbps: f64,
    pub assets_extracted: usize,
    pub allocation_count: u64,
    pub streaming_successful: bool,
    pub errors: Vec<String>,
}

impl BenchmarkMetrics {
    pub fn new(file_size_bytes: u64) -> Self {
        Self {
            start_time: Instant::now(),
            duration: Duration::default(),
            file_size_bytes,
            peak_memory_mb: 0.0,
            throughput_mbps: 0.0,
            assets_extracted: 0,
            allocation_count: 0,
            streaming_successful: false,
            errors: Vec::new(),
        }
    }

    pub fn finish(&mut self) {
        self.duration = self.start_time.elapsed();
        self.calculate_throughput();
    }

    fn calculate_throughput(&mut self) {
        if self.duration.as_secs_f64() > 0.0 {
            let file_size_mb = self.file_size_bytes as f64 / (1024.0 * 1024.0);
            self.throughput_mbps = file_size_mb / self.duration.as_secs_f64();
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StatisticalSummary {
    pub mean_duration_ms: f64,
    pub median_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub mean_throughput_mbps: f64,
    pub median_throughput_mbps: f64,
    pub p95_throughput_mbps: f64,
    pub mean_memory_mb: f64,
    pub median_memory_mb: f64,
    pub p95_memory_mb: f64,
}

/// Performance analysis and scoring
pub struct PerformanceAnalyzer {
    memory_limit_mb: f64,
    throughput_min_mbps: f64,
}

impl PerformanceAnalyzer {
    pub fn new(memory_limit_mb: f64, throughput_min_mbps: f64) -> Self {
        Self {
            memory_limit_mb,
            throughput_min_mbps,
        }
    }

    pub fn analyze_performance(&self, summary: &StatisticalSummary) -> PerformanceAnalysis {
        let memory_score = self.calculate_memory_score(summary.p95_memory_mb);
        let throughput_score = self.calculate_throughput_score(summary.p95_throughput_mbps);
        let consistency_score = self.calculate_consistency_score(summary);

        let overall_score = (memory_score + throughput_score + consistency_score) / 3.0;
        let grade = self.score_to_grade(overall_score);

        PerformanceAnalysis {
            overall_score,
            grade,
            memory_score,
            throughput_score,
            consistency_score,
            memory_within_limit: summary.p95_memory_mb <= self.memory_limit_mb,
            throughput_meets_min: summary.p95_throughput_mbps >= self.throughput_min_mbps,
            recommendations: self.generate_recommendations(summary),
        }
    }

    fn calculate_memory_score(&self, p95_memory_mb: f64) -> f64 {
        let ratio = p95_memory_mb / self.memory_limit_mb;
        if ratio <= 0.6 {
            100.0
        } else if ratio <= 0.8 {
            90.0 - (ratio - 0.6) * 50.0
        } else if ratio <= 1.0 {
            80.0 - (ratio - 0.8) * 100.0
        } else {
            // Penalty for exceeding limit
            0.0_f64.max(60.0 - (ratio - 1.0) * 200.0)
        }
    }

    fn calculate_throughput_score(&self, p95_throughput_mbps: f64) -> f64 {
        let ratio = p95_throughput_mbps / self.throughput_min_mbps;
        if ratio >= 2.0 {
            100.0
        } else if ratio >= 1.5 {
            90.0 + (ratio - 1.5) * 20.0
        } else if ratio >= 1.0 {
            70.0 + (ratio - 1.0) * 40.0
        } else if ratio >= 0.8 {
            50.0 + (ratio - 0.8) * 100.0
        } else {
            0.0_f64.max(ratio * 62.5)
        }
    }

    fn calculate_consistency_score(&self, summary: &StatisticalSummary) -> f64 {
        // Score based on how consistent the measurements are
        let throughput_cv = if summary.mean_throughput_mbps > 0.0 {
            let variance = (summary.p95_throughput_mbps - summary.median_throughput_mbps).abs();
            variance / summary.mean_throughput_mbps
        } else {
            1.0
        };

        let memory_cv = if summary.mean_memory_mb > 0.0 {
            let variance = (summary.p95_memory_mb - summary.median_memory_mb).abs();
            variance / summary.mean_memory_mb
        } else {
            1.0
        };

        let avg_cv = (throughput_cv + memory_cv) / 2.0;
        
        if avg_cv <= 0.1 {
            100.0
        } else if avg_cv <= 0.2 {
            90.0 - (avg_cv - 0.1) * 100.0
        } else if avg_cv <= 0.5 {
            80.0 - (avg_cv - 0.2) * 66.7
        } else {
            0.0_f64.max(60.0 - (avg_cv - 0.5) * 120.0)
        }
    }

    fn score_to_grade(&self, score: f64) -> String {
        match score {
            s if s >= 90.0 => "A".to_string(),
            s if s >= 80.0 => "B".to_string(),
            s if s >= 70.0 => "C".to_string(),
            s if s >= 60.0 => "D".to_string(),
            _ => "F".to_string(),
        }
    }

    fn generate_recommendations(&self, summary: &StatisticalSummary) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Memory recommendations
        if summary.p95_memory_mb > self.memory_limit_mb {
            recommendations.push(format!(
                "Memory usage exceeds limit by {:.1}MB. Consider implementing streaming or reducing buffer sizes.",
                summary.p95_memory_mb - self.memory_limit_mb
            ));
        } else if summary.p95_memory_mb > self.memory_limit_mb * 0.8 {
            recommendations.push(
                "Memory usage is approaching the limit. Monitor for potential issues with larger files.".to_string()
            );
        }

        // Throughput recommendations
        if summary.p95_throughput_mbps < self.throughput_min_mbps {
            let deficit = self.throughput_min_mbps - summary.p95_throughput_mbps;
            recommendations.push(format!(
                "Throughput is {:.1}MB/s below target. Consider optimizing decompression or I/O operations.",
                deficit
            ));
        } else if summary.p95_throughput_mbps < self.throughput_min_mbps * 1.2 {
            recommendations.push(
                "Throughput meets minimum but has little headroom. Consider optimizations for future scalability.".to_string()
            );
        }

        // Consistency recommendations
        let throughput_variance = (summary.p95_throughput_mbps - summary.median_throughput_mbps).abs();
        let memory_variance = (summary.p95_memory_mb - summary.median_memory_mb).abs();

        if throughput_variance > summary.mean_throughput_mbps * 0.2 {
            recommendations.push(
                "High throughput variance detected. Investigate potential bottlenecks or resource contention.".to_string()
            );
        }

        if memory_variance > summary.mean_memory_mb * 0.3 {
            recommendations.push(
                "High memory variance detected. Review memory allocation patterns for efficiency.".to_string()
            );
        }

        if recommendations.is_empty() {
            recommendations.push("Performance looks good! No specific recommendations at this time.".to_string());
        }

        recommendations
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    pub overall_score: f64,
    pub grade: String,
    pub memory_score: f64,
    pub throughput_score: f64,
    pub consistency_score: f64,
    pub memory_within_limit: bool,
    pub throughput_meets_min: bool,
    pub recommendations: Vec<String>,
}

/// Streaming performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingMetrics {
    pub chunks_processed: usize,
    pub avg_chunk_size_kb: f64,
    pub max_buffer_size_mb: f64,
    pub buffer_overflow_count: usize,
    pub back_pressure_events: usize,
}

impl StreamingMetrics {
    pub fn new() -> Self {
        Self {
            chunks_processed: 0,
            avg_chunk_size_kb: 0.0,
            max_buffer_size_mb: 0.0,
            buffer_overflow_count: 0,
            back_pressure_events: 0,
        }
    }

    pub fn record_chunk(&mut self, size_bytes: usize) {
        self.chunks_processed += 1;
        let size_kb = size_bytes as f64 / 1024.0;
        
        // Update running average
        if self.chunks_processed == 1 {
            self.avg_chunk_size_kb = size_kb;
        } else {
            let n = self.chunks_processed as f64;
            self.avg_chunk_size_kb = ((n - 1.0) * self.avg_chunk_size_kb + size_kb) / n;
        }
    }

    pub fn record_buffer_usage(&mut self, size_mb: f64) {
        if size_mb > self.max_buffer_size_mb {
            self.max_buffer_size_mb = size_mb;
        }
    }

    pub fn record_overflow(&mut self) {
        self.buffer_overflow_count += 1;
    }

    pub fn record_back_pressure(&mut self) {
        self.back_pressure_events += 1;
    }

    pub fn is_streaming_successful(&self) -> bool {
        self.buffer_overflow_count == 0 && self.back_pressure_events < self.chunks_processed / 10
    }
}
