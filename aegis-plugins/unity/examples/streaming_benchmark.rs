use anyhow::{bail, Context, Result};
use memmap2::MmapOptions;
use std::fs::File;
use std::hint::black_box;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_SIZE_MB: usize = 64;

#[derive(Clone, Copy)]
enum Mode {
    Read,
    Mmap,
}

impl Mode {
    fn as_str(&self) -> &'static str {
        match self {
            Mode::Read => "std::fs::read",
            Mode::Mmap => "memmap",
        }
    }
}

#[derive(Debug)]
struct MemoryStats {
    vm_rss_kb: u64,
    vm_hwm_kb: u64,
}

#[derive(Debug)]
struct Measurement {
    duration: Duration,
    delta_rss_kb: i64,
    delta_hwm_kb: i64,
    final_stats: MemoryStats,
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    let mut mode = Mode::Mmap;
    let mut path_arg: Option<String> = None;
    let mut size_arg: Option<String> = None;

    if let Some(first) = args.next() {
        match first.as_str() {
            "read" | "std" | "std_read" => {
                mode = Mode::Read;
                path_arg = args.next();
                size_arg = args.next();
            }
            "mmap" | "stream" | "streaming" => {
                mode = Mode::Mmap;
                path_arg = args.next();
                size_arg = args.next();
            }
            other => {
                path_arg = Some(other.to_string());
                size_arg = args.next();
            }
        }
    }

    let size_mb = if let Some(size) = size_arg {
        size.parse::<usize>()
            .context("Failed to parse size argument (MiB)")?
    } else {
        DEFAULT_SIZE_MB
    };

    let size_bytes = size_mb * 1024 * 1024;
    let (path, cleanup) = ensure_benchmark_file(path_arg, size_bytes)?;

    let measurement = match mode {
        Mode::Read => measure_read(&path)?,
        Mode::Mmap => measure_mmap(&path)?,
    };

    println!(
        "Mode: {}\nFile: {} ({} MiB)\nElapsed: {:.2?}\nRSS delta: {} KiB\nPeak delta: {} KiB\nFinal RSS: {} KiB\nPeak RSS: {} KiB",
        mode.as_str(),
        path.display(),
        size_mb,
        measurement.duration,
        measurement.delta_rss_kb,
        measurement.delta_hwm_kb,
        measurement.final_stats.vm_rss_kb,
        measurement.final_stats.vm_hwm_kb,
    );

    if cleanup {
        std::fs::remove_file(&path).ok();
    }

    Ok(())
}

fn ensure_benchmark_file(path_arg: Option<String>, size_bytes: usize) -> Result<(PathBuf, bool)> {
    if let Some(path) = path_arg {
        Ok((PathBuf::from(path), false))
    } else {
        let path = std::env::temp_dir().join(format!(
            "aegis_unity_streaming_bench_{}_{}.bin",
            std::process::id(),
            size_bytes
        ));
        create_sparse_file(&path, size_bytes)?;
        Ok((path, true))
    }
}

fn create_sparse_file(path: &Path, size_bytes: usize) -> Result<()> {
    let mut file = File::create(path)
        .with_context(|| format!("Failed to create benchmark file at {}", path.display()))?;
    file.set_len(size_bytes as u64)
        .with_context(|| format!("Failed to set benchmark file size for {}", path.display()))?;
    file.flush()
        .with_context(|| format!("Failed to flush benchmark file {}", path.display()))?;
    Ok(())
}

fn measure_read(path: &Path) -> Result<Measurement> {
    let before = memory_stats()?;
    let start = Instant::now();
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read benchmark file {}", path.display()))?;
    let sample_len = std::cmp::min(data.len(), 4096);
    black_box(&data[..sample_len]);
    let elapsed = start.elapsed();
    let after = memory_stats()?;
    drop(data);
    Ok(build_measurement(before, after, elapsed))
}

fn measure_mmap(path: &Path) -> Result<Measurement> {
    let before = memory_stats()?;
    let file = File::open(path)
        .with_context(|| format!("Failed to open benchmark file {}", path.display()))?;
    let start = Instant::now();
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .with_context(|| format!("Failed to memory-map {}", path.display()))?
    };
    let sample_len = std::cmp::min(mmap.len(), 4096);
    black_box(&mmap[..sample_len]);
    let elapsed = start.elapsed();
    let after = memory_stats()?;
    drop(mmap);
    Ok(build_measurement(before, after, elapsed))
}

fn build_measurement(before: MemoryStats, after: MemoryStats, duration: Duration) -> Measurement {
    let delta_rss_kb = after.vm_rss_kb as i64 - before.vm_rss_kb as i64;
    let delta_hwm_kb = after.vm_hwm_kb as i64 - before.vm_hwm_kb as i64;
    Measurement {
        duration,
        delta_rss_kb,
        delta_hwm_kb,
        final_stats: after,
    }
}

fn memory_stats() -> Result<MemoryStats> {
    let status = std::fs::read_to_string("/proc/self/status")
        .context("Failed to read /proc/self/status for benchmark")?;

    let mut vm_rss_kb = None;
    let mut vm_hwm_kb = None;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            vm_rss_kb = Some(parse_kib(value)?);
        } else if let Some(value) = line.strip_prefix("VmHWM:") {
            vm_hwm_kb = Some(parse_kib(value)?);
        }
    }

    let vm_rss_kb = vm_rss_kb.context("VmRSS not reported in /proc/self/status")?;
    let vm_hwm_kb = vm_hwm_kb.context("VmHWM not reported in /proc/self/status")?;

    Ok(MemoryStats {
        vm_rss_kb,
        vm_hwm_kb,
    })
}

fn parse_kib(field: &str) -> Result<u64> {
    let mut parts = field.split_whitespace();
    let value = parts
        .next()
        .context("Missing numeric value in memory stat")?;
    let unit = parts.next().unwrap_or("KiB");

    if unit != "kB" && unit != "KiB" {
        bail!("Unexpected memory unit: {}", unit);
    }

    value
        .parse::<u64>()
        .context("Failed to parse memory stat value")
}
