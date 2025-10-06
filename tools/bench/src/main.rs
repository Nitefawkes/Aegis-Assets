use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{info, warn, error};

mod memory;
mod metrics;
mod extraction;
mod corpus;

use crate::memory::MemoryTracker;
use crate::metrics::{BenchmarkMetrics, StatisticalSummary};
use crate::extraction::ExtractionBenchmark;
use crate::corpus::CorpusLoader;

#[derive(Parser)]
#[command(name = "aegis-bench")]
#[command(about = "Performance benchmarking harness for Aegis-Assets extraction pipeline")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Output format (json, yaml, table)
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Output file (stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run benchmark suite on a corpus
    Run {
        /// Path to test corpus directory
        #[arg(short, long)]
        corpus: PathBuf,

        /// Number of iterations per file
        #[arg(short, long, default_value = "3")]
        iterations: u32,

        /// Enable memory profiling
        #[arg(long)]
        profile_memory: bool,

        /// Enable streaming validation
        #[arg(long)]
        validate_streaming: bool,

        /// Memory limit for streaming tests (MB)
        #[arg(long, default_value = "300")]
        memory_limit: usize,

        /// Minimum throughput threshold (MB/s)
        #[arg(long, default_value = "120")]
        throughput_min: f64,
    },

    /// Validate corpus integrity
    Validate {
        /// Path to test corpus directory  
        #[arg(short, long)]
        corpus: PathBuf,
    },

    /// Generate performance report
    Report {
        /// Path to benchmark results JSON
        #[arg(short, long)]
        results: PathBuf,

        /// Include detailed statistics
        #[arg(long)]
        detailed: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkResults {
    timestamp: String,
    version: String,
    corpus_path: String,
    system_info: SystemInfo,
    file_results: Vec<FileResult>,
    summary: BenchmarkSummary,
}

#[derive(Debug, Serialize, Deserialize)]
struct SystemInfo {
    os: String,
    arch: String,
    cpu_cores: usize,
    total_memory_gb: f64,
    aegis_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileResult {
    filename: String,
    file_size_bytes: u64,
    iterations: Vec<IterationResult>,
    statistics: StatisticalSummary,
}

#[derive(Debug, Serialize, Deserialize)]
struct IterationResult {
    duration_ms: u64,
    peak_memory_mb: f64,
    throughput_mbps: f64,
    allocation_count: u64,
    assets_extracted: usize,
    streaming_successful: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkSummary {
    total_files: usize,
    total_iterations: usize,
    total_processing_time_ms: u64,
    avg_throughput_mbps: f64,
    p95_memory_mb: f64,
    p95_throughput_mbps: f64,
    streaming_success_rate: f64,
    performance_grade: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("aegis_bench={}", log_level))
        .init();

    match cli.command {
        Commands::Run {
            corpus,
            iterations,
            profile_memory,
            validate_streaming,
            memory_limit,
            throughput_min,
        } => {
            run_benchmark(
                corpus,
                iterations,
                profile_memory,
                validate_streaming,
                memory_limit,
                throughput_min,
                &cli.format,
                cli.output,
            ).await
        }
        Commands::Validate { corpus } => {
            validate_corpus(corpus).await
        }
        Commands::Report { results, detailed } => {
            generate_report(results, detailed, &cli.format, cli.output).await
        }
    }
}

async fn run_benchmark(
    corpus_path: PathBuf,
    iterations: u32,
    profile_memory: bool,
    validate_streaming: bool,
    memory_limit: usize,
    throughput_min: f64,
    format: &str,
    output: Option<PathBuf>,
) -> Result<()> {
    info!("Starting benchmark run on corpus: {}", corpus_path.display());

    // Load corpus
    let corpus = CorpusLoader::new(corpus_path.clone())?;
    let files = corpus.load_test_files().await?;

    if files.is_empty() {
        warn!("No test files found in corpus");
        return Ok(());
    }

    info!("Found {} test files in corpus", files.len());

    // Initialize system info
    let system_info = collect_system_info()?;
    
    // Setup memory tracker
    let mut memory_tracker = MemoryTracker::new(profile_memory);
    memory_tracker.start_monitoring();

    // Initialize benchmark engine
    let mut benchmark = ExtractionBenchmark::new(
        memory_limit,
        validate_streaming,
    )?;

    let mut file_results = Vec::new();
    let start_time = Instant::now();

    // Run benchmarks on each file
    for file_path in files {
        info!("Benchmarking file: {}", file_path.display());

        let file_size = std::fs::metadata(&file_path)
            .context("Failed to get file metadata")?
            .len();

        let mut iteration_results = Vec::new();

        // Run multiple iterations
        for iteration in 0..iterations {
            info!("  Iteration {}/{}", iteration + 1, iterations);

            memory_tracker.reset();
            let iter_start = Instant::now();

            // Run extraction benchmark
            let result = benchmark.run_extraction(&file_path).await?;

            let duration = iter_start.elapsed();
            let peak_memory = memory_tracker.peak_memory_mb();
            let throughput = (file_size as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();

            let iter_result = IterationResult {
                duration_ms: duration.as_millis() as u64,
                peak_memory_mb: peak_memory,
                throughput_mbps: throughput,
                allocation_count: memory_tracker.allocation_count(),
                assets_extracted: result.assets_extracted,
                streaming_successful: result.streaming_successful,
            };

            iteration_results.push(iter_result);

            // Check for memory limit violations
            if validate_streaming && peak_memory > memory_limit as f64 {
                warn!("Memory limit exceeded: {:.1}MB > {}MB", peak_memory, memory_limit);
            }

            // Check for throughput threshold
            if throughput < throughput_min {
                warn!("Throughput below threshold: {:.1}MB/s < {:.1}MB/s", throughput, throughput_min);
            }
        }

        // Calculate statistics for this file
        let statistics = calculate_statistics(&iteration_results);
        
        let file_result = FileResult {
            filename: file_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_size_bytes: file_size,
            iterations: iteration_results,
            statistics,
        };

        file_results.push(file_result);
    }

    let total_time = start_time.elapsed();

    // Calculate overall summary
    let summary = calculate_summary(&file_results, total_time, throughput_min, memory_limit as f64);

    let results = BenchmarkResults {
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        corpus_path: corpus_path.to_string_lossy().to_string(),
        system_info,
        file_results,
        summary,
    };

    // Output results
    output_results(&results, format, output).await?;

    // Report performance grade
    match results.summary.performance_grade.as_str() {
        "A" | "B" => info!("✅ Performance grade: {}", results.summary.performance_grade),
        "C" => warn!("⚠️ Performance grade: {}", results.summary.performance_grade),
        _ => error!("❌ Performance grade: {}", results.summary.performance_grade),
    }

    Ok(())
}

async fn validate_corpus(corpus_path: PathBuf) -> Result<()> {
    info!("Validating corpus: {}", corpus_path.display());

    let corpus = CorpusLoader::new(corpus_path)?;
    corpus.validate().await?;

    info!("✅ Corpus validation successful");
    Ok(())
}

async fn generate_report(
    results_path: PathBuf,
    detailed: bool,
    format: &str,
    output: Option<PathBuf>,
) -> Result<()> {
    info!("Generating report from: {}", results_path.display());

    let results_content = std::fs::read_to_string(&results_path)
        .context("Failed to read results file")?;
    
    let results: BenchmarkResults = serde_json::from_str(&results_content)
        .context("Failed to parse results JSON")?;

    // TODO: Implement report generation
    info!("Report generation not yet implemented");
    
    Ok(())
}

fn collect_system_info() -> Result<SystemInfo> {
    use sysinfo::{System, SystemExt};

    let system = System::new_all();
    
    Ok(SystemInfo {
        os: system.name().unwrap_or_else(|| "unknown".to_string()),
        arch: std::env::consts::ARCH.to_string(),
        cpu_cores: system.processors().len(),
        total_memory_gb: system.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0),
        aegis_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

fn calculate_statistics(iterations: &[IterationResult]) -> StatisticalSummary {
    if iterations.is_empty() {
        return StatisticalSummary::default();
    }

    let durations: Vec<f64> = iterations.iter().map(|i| i.duration_ms as f64).collect();
    let throughputs: Vec<f64> = iterations.iter().map(|i| i.throughput_mbps).collect();
    let memories: Vec<f64> = iterations.iter().map(|i| i.peak_memory_mb).collect();

    StatisticalSummary {
        mean_duration_ms: mean(&durations),
        median_duration_ms: median(&durations),
        p95_duration_ms: percentile(&durations, 0.95),
        mean_throughput_mbps: mean(&throughputs),
        median_throughput_mbps: median(&throughputs),
        p95_throughput_mbps: percentile(&throughputs, 0.95),
        mean_memory_mb: mean(&memories),
        median_memory_mb: median(&memories),
        p95_memory_mb: percentile(&memories, 0.95),
    }
}

fn calculate_summary(
    file_results: &[FileResult],
    total_time: Duration,
    throughput_min: f64,
    memory_limit: f64,
) -> BenchmarkSummary {
    let total_iterations: usize = file_results.iter()
        .map(|f| f.iterations.len())
        .sum();

    let all_throughputs: Vec<f64> = file_results.iter()
        .flat_map(|f| f.iterations.iter().map(|i| i.throughput_mbps))
        .collect();

    let all_memories: Vec<f64> = file_results.iter()
        .flat_map(|f| f.iterations.iter().map(|i| i.peak_memory_mb))
        .collect();

    let streaming_successes: usize = file_results.iter()
        .flat_map(|f| f.iterations.iter())
        .map(|i| if i.streaming_successful { 1 } else { 0 })
        .sum();

    let avg_throughput = mean(&all_throughputs);
    let p95_memory = percentile(&all_memories, 0.95);
    let p95_throughput = percentile(&all_throughputs, 0.95);
    let streaming_success_rate = streaming_successes as f64 / total_iterations as f64;

    // Calculate performance grade
    let grade = if p95_throughput >= throughput_min && p95_memory <= memory_limit {
        if avg_throughput >= throughput_min * 1.5 && p95_memory <= memory_limit * 0.8 {
            "A"
        } else {
            "B"
        }
    } else if p95_throughput >= throughput_min * 0.8 || p95_memory <= memory_limit * 1.2 {
        "C"
    } else {
        "F"
    };

    BenchmarkSummary {
        total_files: file_results.len(),
        total_iterations,
        total_processing_time_ms: total_time.as_millis() as u64,
        avg_throughput_mbps: avg_throughput,
        p95_memory_mb: p95_memory,
        p95_throughput_mbps: p95_throughput,
        streaming_success_rate,
        performance_grade: grade.to_string(),
    }
}

async fn output_results(
    results: &BenchmarkResults,
    format: &str,
    output: Option<PathBuf>,
) -> Result<()> {
    let content = match format {
        "json" => serde_json::to_string_pretty(results)?,
        "yaml" => serde_yaml::to_string(results)?,
        "table" => format_table_output(results),
        _ => return Err(anyhow::anyhow!("Unsupported output format: {}", format)),
    };

    match output {
        Some(path) => {
            std::fs::write(&path, content)
                .context("Failed to write output file")?;
            info!("Results written to: {}", path.display());
        }
        None => {
            println!("{}", content);
        }
    }

    Ok(())
}

fn format_table_output(results: &BenchmarkResults) -> String {
    use std::fmt::Write;
    
    let mut output = String::new();
    
    writeln!(output, "🛡️ Aegis-Assets Benchmark Results").unwrap();
    writeln!(output, "=====================================").unwrap();
    writeln!(output, "Timestamp: {}", results.timestamp).unwrap();
    writeln!(output, "Corpus: {}", results.corpus_path).unwrap();
    writeln!(output, "System: {} {} ({} cores, {:.1}GB RAM)", 
        results.system_info.os, 
        results.system_info.arch,
        results.system_info.cpu_cores,
        results.system_info.total_memory_gb).unwrap();
    writeln!(output).unwrap();

    writeln!(output, "📊 Summary").unwrap();
    writeln!(output, "-----------").unwrap();
    writeln!(output, "Performance Grade: {}", results.summary.performance_grade).unwrap();
    writeln!(output, "Files Processed: {}", results.summary.total_files).unwrap();
    writeln!(output, "Total Iterations: {}", results.summary.total_iterations).unwrap();
    writeln!(output, "Average Throughput: {:.1} MB/s", results.summary.avg_throughput_mbps).unwrap();
    writeln!(output, "P95 Throughput: {:.1} MB/s", results.summary.p95_throughput_mbps).unwrap();
    writeln!(output, "P95 Memory Usage: {:.1} MB", results.summary.p95_memory_mb).unwrap();
    writeln!(output, "Streaming Success Rate: {:.1}%", results.summary.streaming_success_rate * 100.0).unwrap();
    writeln!(output).unwrap();

    writeln!(output, "📁 File Results").unwrap();
    writeln!(output, "---------------").unwrap();
    writeln!(output, "{:<30} {:>10} {:>15} {:>15} {:>15}", 
        "File", "Size (MB)", "Avg Time (ms)", "Avg Throughput", "P95 Memory").unwrap();
    
    for file_result in &results.file_results {
        let size_mb = file_result.file_size_bytes as f64 / (1024.0 * 1024.0);
        writeln!(output, "{:<30} {:>10.1} {:>15.0} {:>15.1} {:>15.1}",
            file_result.filename,
            size_mb,
            file_result.statistics.mean_duration_ms,
            file_result.statistics.mean_throughput_mbps,
            file_result.statistics.p95_memory_mb).unwrap();
    }

    output
}

// Statistical helper functions
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 }
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() { return 0.0; }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let index = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}
