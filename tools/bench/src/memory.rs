use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(all(feature = "detailed_memory", not(target_os = "windows")))]
use jemalloc_ctl::{epoch, stats};

/// Memory tracking for benchmark measurements
pub struct MemoryTracker {
    enabled: bool,
    peak_memory: Arc<AtomicU64>,
    allocation_count: Arc<AtomicU64>,
    start_memory: u64,
    monitoring: bool,
}

impl MemoryTracker {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            peak_memory: Arc::new(AtomicU64::new(0)),
            allocation_count: Arc::new(AtomicU64::new(0)),
            start_memory: 0,
            monitoring: false,
        }
    }

    pub fn start_monitoring(&mut self) {
        if !self.enabled {
            return;
        }

        self.start_memory = self.current_memory_usage();
        self.peak_memory.store(self.start_memory, Ordering::SeqCst);
        self.allocation_count.store(0, Ordering::SeqCst);
        self.monitoring = true;
    }

    pub fn reset(&mut self) {
        if !self.enabled {
            return;
        }

        self.start_memory = self.current_memory_usage();
        self.peak_memory.store(self.start_memory, Ordering::SeqCst);
        self.allocation_count.store(0, Ordering::SeqCst);
    }

    pub fn update(&self) {
        if !self.enabled || !self.monitoring {
            return;
        }

        let current = self.current_memory_usage();
        let current_peak = self.peak_memory.load(Ordering::SeqCst);
        
        if current > current_peak {
            self.peak_memory.store(current, Ordering::SeqCst);
        }

        // Increment allocation count (simplified tracking)
        self.allocation_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn peak_memory_mb(&self) -> f64 {
        if !self.enabled {
            return 0.0;
        }

        let peak_bytes = self.peak_memory.load(Ordering::SeqCst);
        peak_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn allocation_count(&self) -> u64 {
        if !self.enabled {
            return 0;
        }

        self.allocation_count.load(Ordering::SeqCst)
    }

    #[cfg(target_os = "windows")]
    fn current_memory_usage(&self) -> u64 {
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
        use std::mem;

        unsafe {
            let process = GetCurrentProcess();
            let mut pmc: PROCESS_MEMORY_COUNTERS = mem::zeroed();
            
            if winapi::um::psapi::GetProcessMemoryInfo(
                process,
                &mut pmc,
                mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ) != 0 {
                pmc.WorkingSetSize as u64
            } else {
                0
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn current_memory_usage(&self) -> u64 {
        use procfs::process::Process;

        if let Ok(process) = Process::myself() {
            if let Ok(stat) = process.stat() {
                return stat.rss as u64 * 4096; // RSS is in pages, typically 4KB each
            }
        }
        0
    }

    #[cfg(target_os = "macos")]
    fn current_memory_usage(&self) -> u64 {
        use libc::{getpid, mach_task_self, task_info, task_basic_info, TASK_BASIC_INFO};
        use std::mem;

        unsafe {
            let mut info: task_basic_info = mem::zeroed();
            let mut count = TASK_BASIC_INFO_COUNT;
            
            let result = task_info(
                mach_task_self(),
                TASK_BASIC_INFO,
                &mut info as *mut _ as *mut _,
                &mut count,
            );

            if result == 0 {
                info.resident_size as u64
            } else {
                0
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    fn current_memory_usage(&self) -> u64 {
        // Fallback for unsupported platforms
        0
    }
}

/// Memory pressure monitoring for streaming tests
pub struct MemoryPressureMonitor {
    memory_limit_bytes: u64,
    check_interval: Duration,
    tracker: MemoryTracker,
}

impl MemoryPressureMonitor {
    pub fn new(memory_limit_mb: usize) -> Self {
        Self {
            memory_limit_bytes: (memory_limit_mb as u64) * 1024 * 1024,
            check_interval: Duration::from_millis(100),
            tracker: MemoryTracker::new(true),
        }
    }

    pub fn start(&mut self) {
        self.tracker.start_monitoring();
    }

    pub fn check_pressure(&self) -> MemoryPressureStatus {
        let current_mb = self.tracker.peak_memory_mb();
        let limit_mb = self.memory_limit_bytes as f64 / (1024.0 * 1024.0);
        let pressure = current_mb / limit_mb;

        if pressure > 1.0 {
            MemoryPressureStatus::Critical { current_mb, limit_mb }
        } else if pressure > 0.8 {
            MemoryPressureStatus::High { current_mb, limit_mb, pressure }
        } else if pressure > 0.6 {
            MemoryPressureStatus::Medium { current_mb, pressure }
        } else {
            MemoryPressureStatus::Low { current_mb }
        }
    }

    pub fn peak_memory_mb(&self) -> f64 {
        self.tracker.peak_memory_mb()
    }
}

#[derive(Debug, Clone)]
pub enum MemoryPressureStatus {
    Low { current_mb: f64 },
    Medium { current_mb: f64, pressure: f64 },
    High { current_mb: f64, limit_mb: f64, pressure: f64 },
    Critical { current_mb: f64, limit_mb: f64 },
}

impl MemoryPressureStatus {
    pub fn is_critical(&self) -> bool {
        matches!(self, MemoryPressureStatus::Critical { .. })
    }

    pub fn is_high_pressure(&self) -> bool {
        matches!(self, MemoryPressureStatus::High { .. } | MemoryPressureStatus::Critical { .. })
    }
}

/// Simple allocation tracker for basic allocation counting
pub struct AllocationTracker {
    count: Arc<AtomicU64>,
    enabled: bool,
}

impl AllocationTracker {
    pub fn new(enabled: bool) -> Self {
        Self {
            count: Arc::new(AtomicU64::new(0)),
            enabled,
        }
    }

    pub fn track_allocation(&self, _size: usize) {
        if self.enabled {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn allocation_count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
    }
}
