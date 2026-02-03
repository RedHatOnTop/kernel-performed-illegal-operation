# Sub-Phase 5.4: Performance Optimization Checklist

## Overview
Optimize system performance across all components for smooth user experience.

---

## Performance Targets

| Metric | Current | Target | Priority |
|--------|---------|--------|----------|
| Cold Boot Time | TBD | < 3 seconds | Critical |
| Warm Boot Time | TBD | < 1.5 seconds | High |
| Memory (Idle) | TBD | < 256 MB | Critical |
| Memory (10 tabs) | TBD | < 512 MB | High |
| First Paint | TBD | < 100 ms | High |
| Input Latency | TBD | < 16 ms | Critical |
| Page Load | TBD | < 1 second | Medium |
| App Launch | TBD | < 500 ms | High |
| FPS (Desktop) | TBD | 60 FPS | High |
| CPU Idle | TBD | < 2% | Medium |

---

## 5.4.1 Memory Optimization

### Tasks

| ID | Task | Description | Expected Gain | Status |
|----|------|-------------|---------------|--------|
| MEM001 | Memory compression | Implement zswap-like compression | -30% memory | ⬜ |
| MEM002 | Page reclamation | Reclaim inactive pages | -20% memory | ⬜ |
| MEM003 | Slab tuning | Optimize slab cache sizes | -5% memory | ⬜ |
| MEM004 | Heap defrag | Reduce fragmentation | -10% memory | ⬜ |
| MEM005 | COW fork | Copy-on-write for fork() | -40% fork | ⬜ |
| MEM006 | Tab suspension | Suspend inactive tabs | -50% browser | ⬜ |
| MEM007 | Image caching | Smart image cache | -20% browser | ⬜ |
| MEM008 | String interning | Intern common strings | -5% overall | ⬜ |

### Implementation Details

#### MEM001: Memory Compression
```rust
// kernel/src/memory/compression.rs

pub struct CompressedPage {
    compressed_data: Vec<u8>,
    original_size: usize,
    compression_ratio: f32,
}

impl MemoryCompressor {
    /// Compress a page using LZ4
    pub fn compress(&self, page: &Page) -> CompressedPage;
    
    /// Decompress on access
    pub fn decompress(&self, compressed: &CompressedPage) -> Page;
    
    /// Background compression of cold pages
    pub fn scan_and_compress(&mut self);
}
```

#### MEM002: Page Reclamation
```rust
// kernel/src/memory/reclaim.rs

pub struct PageReclaimer {
    lru_list: LruList,
    watermark_high: usize,
    watermark_low: usize,
}

impl PageReclaimer {
    /// Check if reclamation needed
    pub fn should_reclaim(&self) -> bool;
    
    /// Reclaim inactive pages
    pub fn reclaim(&mut self, target_pages: usize) -> usize;
    
    /// Add page to LRU
    pub fn access_page(&mut self, page: PhysAddr);
}
```

### Memory Profiling

```bash
# Memory breakdown at idle
./scripts/profile_memory.sh

# Expected output:
# Kernel Code:     8 MB
# Kernel Heap:    16 MB  
# Page Tables:     4 MB
# Driver Buffers: 32 MB
# Browser:        64 MB
# Apps:           32 MB
# File Cache:     64 MB
# Free:          100 MB
# ─────────────────────
# Total:         320 MB → Target: 256 MB
```

---

## 5.4.2 CPU Optimization

### Tasks

| ID | Task | Description | Expected Gain | Status |
|----|------|-------------|---------------|--------|
| CPU001 | Scheduler tuning | Optimize time slice | -10% latency | ⬜ |
| CPU002 | CPU affinity | Pin tasks to cores | -15% cache miss | ⬜ |
| CPU003 | NUMA awareness | Local memory access | -20% latency | ⬜ |
| CPU004 | Context switch | Reduce switch overhead | -5% CPU | ⬜ |
| CPU005 | Tickless idle | No timer in idle | -30% power | ⬜ |
| CPU006 | Syscall fastpath | Optimize hot syscalls | -10% overhead | ⬜ |
| CPU007 | Lock-free data | Use atomic operations | -15% contention | ⬜ |
| CPU008 | Batch processing | Batch small operations | -10% overhead | ⬜ |

### Implementation Details

#### CPU001: Scheduler Tuning
```rust
// kernel/src/scheduler/tuning.rs

pub const SCHED_PARAMS: SchedulerParams = SchedulerParams {
    // Interactive tasks (UI, input)
    interactive_timeslice_us: 1_000,     // 1ms
    interactive_priority_boost: 5,
    
    // Normal tasks
    normal_timeslice_us: 10_000,         // 10ms
    
    // Background tasks (indexing, compression)
    background_timeslice_us: 50_000,     // 50ms
    background_priority: -5,
    
    // Preemption
    min_granularity_us: 750,             // Min 750µs before preempt
    wakeup_preempt_threshold_us: 500,
};
```

#### CPU005: Tickless Idle
```rust
// kernel/src/scheduler/tickless.rs

impl TicklessScheduler {
    /// Enter tickless mode when no tasks runnable
    pub fn enter_idle(&mut self) {
        let next_event = self.next_timer_event();
        let deadline = next_event.min(MAX_IDLE_TIME);
        
        // Stop periodic tick
        self.disable_tick();
        
        // Set one-shot timer for next event
        self.set_wakeup_timer(deadline);
        
        // Enter low-power state
        cpu::halt();
    }
    
    /// Resume from idle
    pub fn exit_idle(&mut self) {
        self.enable_tick();
    }
}
```

### CPU Profiling

```bash
# CPU profile during idle
./scripts/profile_cpu.sh --idle

# CPU profile during workload
./scripts/profile_cpu.sh --workload browser

# Flame graph generation
./scripts/generate_flamegraph.sh
```

---

## 5.4.3 Graphics Optimization

### Tasks

| ID | Task | Description | Expected Gain | Status |
|----|------|-------------|---------------|--------|
| GFX001 | Tile rendering | Render in tiles | -30% overdraw | ⬜ |
| GFX002 | GPU batching | Batch draw commands | -50% draw calls | ⬜ |
| GFX003 | Damage tracking | Only redraw changed | -80% redraws | ⬜ |
| GFX004 | Texture atlas | Combine small textures | -40% binds | ⬜ |
| GFX005 | Frame pacing | Consistent frame timing | Smooth 60fps | ⬜ |
| GFX006 | Hardware cursor | GPU cursor rendering | -5% CPU | ⬜ |
| GFX007 | Layer caching | Cache static layers | -60% redraws | ⬜ |
| GFX008 | Font caching | Cache glyph rasterization | -70% text | ⬜ |

### Implementation Details

#### GFX003: Damage Tracking
```rust
// graphics/src/damage.rs

pub struct DamageTracker {
    regions: Vec<Rect>,
    full_damage: bool,
}

impl DamageTracker {
    /// Mark region as damaged
    pub fn add_damage(&mut self, rect: Rect) {
        if !self.full_damage {
            self.regions.push(rect);
            self.merge_overlapping();
        }
    }
    
    /// Get damage regions for rendering
    pub fn get_damage(&self) -> &[Rect] {
        &self.regions
    }
    
    /// Clear damage after render
    pub fn clear(&mut self) {
        self.regions.clear();
        self.full_damage = false;
    }
}
```

#### GFX005: Frame Pacing
```rust
// graphics/src/pacing.rs

pub struct FramePacer {
    target_fps: u32,
    frame_time_ns: u64,
    last_frame: Instant,
    frame_times: CircularBuffer<u64, 60>,
}

impl FramePacer {
    pub fn new(fps: u32) -> Self {
        Self {
            target_fps: fps,
            frame_time_ns: 1_000_000_000 / fps as u64,
            last_frame: Instant::now(),
            frame_times: CircularBuffer::new(),
        }
    }
    
    /// Wait for next frame
    pub fn wait_for_frame(&mut self) {
        let elapsed = self.last_frame.elapsed().as_nanos() as u64;
        if elapsed < self.frame_time_ns {
            let wait = self.frame_time_ns - elapsed;
            sleep(Duration::from_nanos(wait));
        }
        self.last_frame = Instant::now();
    }
    
    /// Get current FPS
    pub fn current_fps(&self) -> f32 {
        let avg = self.frame_times.average();
        1_000_000_000.0 / avg as f32
    }
}
```

### Graphics Profiling

```bash
# FPS monitoring
./scripts/monitor_fps.sh

# GPU utilization
./scripts/gpu_stats.sh

# Frame timing analysis
./scripts/frame_analyzer.sh
```

---

## 5.4.4 I/O Optimization

### Tasks

| ID | Task | Description | Expected Gain | Status |
|----|------|-------------|---------------|--------|
| IO001 | io_uring tuning | Optimize ring params | -20% latency | ⬜ |
| IO002 | Read-ahead | Prefetch sequential | +50% throughput | ⬜ |
| IO003 | Write coalescing | Batch small writes | -30% IOPS | ⬜ |
| IO004 | Zero-copy network | Avoid data copies | -50% CPU | ⬜ |
| IO005 | Async DNS | Non-blocking DNS | -100ms latency | ⬜ |
| IO006 | Connection pool | Reuse TCP connections | -200ms per req | ⬜ |
| IO007 | Compression | Gzip content encoding | -60% bandwidth | ⬜ |
| IO008 | Priority I/O | Prioritize interactive | -50% UI latency | ⬜ |

### Implementation Details

#### IO002: Read-ahead
```rust
// kernel/src/io/readahead.rs

pub struct ReadAhead {
    window_size: usize,
    trigger_threshold: usize,
    pattern: AccessPattern,
}

impl ReadAhead {
    /// Detect sequential access pattern
    pub fn record_access(&mut self, offset: u64, len: usize) {
        if offset == self.last_offset + self.last_len as u64 {
            self.sequential_count += 1;
            if self.sequential_count > self.trigger_threshold {
                self.pattern = AccessPattern::Sequential;
                self.schedule_readahead(offset + len as u64);
            }
        } else {
            self.sequential_count = 0;
            self.pattern = AccessPattern::Random;
        }
    }
    
    /// Schedule background read
    fn schedule_readahead(&self, offset: u64) {
        io_submit_async(ReadRequest {
            offset,
            len: self.window_size,
            priority: Priority::Background,
        });
    }
}
```

#### IO006: Connection Pool
```rust
// kpio-browser/src/network/pool.rs

pub struct ConnectionPool {
    connections: HashMap<HostPort, Vec<Connection>>,
    max_per_host: usize,
    idle_timeout: Duration,
}

impl ConnectionPool {
    /// Get or create connection
    pub async fn get(&mut self, host: &str, port: u16) -> Connection {
        let key = HostPort::new(host, port);
        
        // Try to reuse existing
        if let Some(conns) = self.connections.get_mut(&key) {
            if let Some(conn) = conns.pop() {
                if !conn.is_stale() {
                    return conn;
                }
            }
        }
        
        // Create new
        Connection::new(host, port).await
    }
    
    /// Return connection to pool
    pub fn release(&mut self, conn: Connection) {
        let key = conn.host_port();
        let entry = self.connections.entry(key).or_default();
        if entry.len() < self.max_per_host {
            entry.push(conn);
        }
    }
}
```

---

## 5.4.5 Profiling Tools

### Built-in Profiler
```rust
// kernel/src/profiling/mod.rs

pub struct Profiler {
    traces: Vec<TraceEntry>,
    metrics: HashMap<String, Metric>,
}

impl Profiler {
    /// Start trace span
    pub fn start_span(&mut self, name: &str) -> SpanGuard {
        SpanGuard::new(self, name)
    }
    
    /// Record metric
    pub fn record(&mut self, name: &str, value: u64) {
        self.metrics
            .entry(name.into())
            .or_default()
            .record(value);
    }
    
    /// Generate report
    pub fn report(&self) -> ProfileReport {
        ProfileReport {
            traces: self.aggregate_traces(),
            metrics: self.metrics.clone(),
            timestamp: Instant::now(),
        }
    }
}

// Usage
profiler::start_span("page_render");
render_page()?;
profiler::end_span();
```

### Benchmark Suite

```bash
# Full benchmark suite
./scripts/benchmark.sh

# Individual benchmarks
cargo bench -p kpio-kernel -- memory
cargo bench -p kpio-browser -- render
cargo bench -p kpio-html -- parse

# Results comparison
./scripts/compare_benchmarks.sh baseline.json current.json
```

### Benchmark Results Template

```
KPIO OS Performance Benchmark
Date: ____________
Commit: __________

┌─────────────────────┬──────────┬──────────┬──────────┐
│ Benchmark           │ Baseline │ Current  │ Change   │
├─────────────────────┼──────────┼──────────┼──────────┤
│ Boot Time           │ _____ ms │ _____ ms │ _____ %  │
│ Memory Idle         │ _____ MB │ _____ MB │ _____ %  │
│ First Paint         │ _____ ms │ _____ ms │ _____ %  │
│ Page Load           │ _____ ms │ _____ ms │ _____ %  │
│ App Launch          │ _____ ms │ _____ ms │ _____ %  │
│ FPS (60fps target)  │ _____ fps│ _____ fps│ _____ %  │
│ Input Latency       │ _____ ms │ _____ ms │ _____ %  │
└─────────────────────┴──────────┴──────────┴──────────┘
```

---

## Optimization Checklist

### Before Optimization
- [ ] Establish baseline measurements
- [ ] Identify bottlenecks with profiling
- [ ] Document current performance
- [ ] Set target improvements

### During Optimization
- [ ] Change one thing at a time
- [ ] Measure after each change
- [ ] Verify no regressions
- [ ] Document changes

### After Optimization
- [ ] Compare to baseline
- [ ] Verify targets met
- [ ] Update benchmarks
- [ ] Document final results

---

## Acceptance Criteria

- [ ] Boot time < 3 seconds
- [ ] Memory (idle) < 256 MB
- [ ] First paint < 100 ms
- [ ] Input latency < 16 ms
- [ ] Consistent 60 FPS
- [ ] No performance regressions

---

## Sign-off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | | | |
| Reviewer | | | |
| QA | | | |
