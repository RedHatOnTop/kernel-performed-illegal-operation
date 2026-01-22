# Phase 4: 폴리시 (Polish) 설계 문서

## 개요

Phase 4는 KPIO 운영체제의 완성도를 높이는 단계입니다. 멀티코어(SMP) 지원, 성능 최적화, 윈도우 매니저 완성, 추가 드라이버 지원, 그리고 전체 문서 정리를 포함합니다.

---

## 선행 조건

- Phase 3 완료 (그래픽, 컴포지터, 입력)

## 완료 조건

- 멀티코어에서 WASM 태스크 분산 실행
- 윈도우 매니저 UI 완성
- 성능 벤치마크 통과

---

## 1. 멀티코어 (SMP) 지원

### 1.1 AP 부팅

```rust
// kernel/src/smp/ap_boot.rs

//! Application Processor 부팅
//! 
//! BSP에서 SIPI를 전송하여 AP 깨우기

use x86_64::PhysAddr;
use core::sync::atomic::{AtomicU32, Ordering};

/// AP 부팅 트램폴린 (16비트 실모드 코드)
/// 
/// 0x8000 물리 주소에 복사되어 실행됨
#[repr(align(4096))]
struct ApTrampoline {
    code: [u8; 4096],
}

/// 활성화된 CPU 수
static AP_COUNT: AtomicU32 = AtomicU32::new(0);

/// SMP 초기화
pub fn init_smp(acpi_madt: &Madt) -> usize {
    let bsp_id = get_bsp_apic_id();
    let mut cpu_count = 1; // BSP 포함
    
    // 트램폴린 코드 복사
    unsafe {
        let trampoline_src = include_bytes!("ap_trampoline.bin");
        let trampoline_dst = 0x8000 as *mut u8;
        core::ptr::copy_nonoverlapping(
            trampoline_src.as_ptr(),
            trampoline_dst,
            trampoline_src.len(),
        );
        
        // GDT 포인터, 페이지 테이블 주소 등 패치
        patch_trampoline(trampoline_dst);
    }
    
    // 각 AP에 SIPI 전송
    for entry in acpi_madt.entries() {
        if let MadtEntry::LocalApic(lapic) = entry {
            if lapic.apic_id == bsp_id {
                continue; // BSP 스킵
            }
            
            if lapic.flags & 1 == 0 {
                continue; // 비활성 CPU 스킵
            }
            
            boot_ap(lapic.apic_id);
            cpu_count += 1;
        }
    }
    
    // 모든 AP가 시작될 때까지 대기
    let expected = cpu_count - 1;
    while AP_COUNT.load(Ordering::Acquire) < expected as u32 {
        core::hint::spin_loop();
    }
    
    log::info!("SMP initialized: {} CPUs active", cpu_count);
    cpu_count
}

/// AP 부팅
fn boot_ap(apic_id: u8) {
    let local_apic = get_local_apic();
    
    // INIT IPI
    local_apic.send_ipi(
        apic_id,
        IpiDeliveryMode::Init,
        0,
    );
    
    // 10ms 대기
    sleep_ms(10);
    
    // SIPI (Startup IPI) x2
    for _ in 0..2 {
        local_apic.send_ipi(
            apic_id,
            IpiDeliveryMode::Startup,
            0x08, // 0x8000 >> 12
        );
        sleep_us(200);
    }
}

/// AP 엔트리 포인트 (C에서 호출됨)
#[no_mangle]
pub extern "C" fn ap_entry() -> ! {
    // GDT 로드
    load_gdt();
    
    // IDT 로드
    load_idt();
    
    // 페이지 테이블 활성화
    enable_paging();
    
    // Local APIC 초기화
    init_local_apic();
    
    // CPU 등록
    let cpu_id = get_current_cpu_id();
    AP_COUNT.fetch_add(1, Ordering::Release);
    
    log::debug!("AP {} online", cpu_id);
    
    // 스케줄러에 합류
    scheduler::join_ap(cpu_id)
}
```

### 1.2 CPU별 데이터 구조

```rust
// kernel/src/smp/percpu.rs

//! CPU별 데이터 구조

use alloc::boxed::Box;
use core::cell::UnsafeCell;

/// 최대 CPU 수
pub const MAX_CPUS: usize = 256;

/// CPU별 데이터
#[repr(C)]
pub struct PerCpu {
    /// CPU ID
    pub cpu_id: u32,
    /// APIC ID
    pub apic_id: u8,
    /// 현재 실행 중인 태스크
    pub current_task: Option<TaskId>,
    /// 이 CPU의 런큐
    pub run_queue: RunQueue,
    /// 유휴 태스크
    pub idle_task: TaskId,
    /// 커널 스택 (인터럽트용)
    pub kernel_stack: u64,
    /// TSS
    pub tss: TaskStateSegment,
    /// 타이머 틱
    pub timer_ticks: u64,
    /// 통계
    pub stats: CpuStats,
}

/// CPU 통계
#[derive(Default)]
pub struct CpuStats {
    /// 컨텍스트 스위치 횟수
    pub context_switches: u64,
    /// 인터럽트 횟수
    pub interrupts: u64,
    /// 유휴 시간 (ns)
    pub idle_time: u64,
    /// 사용자 시간 (ns)
    pub user_time: u64,
    /// 시스템 시간 (ns)
    pub system_time: u64,
}

/// 전역 CPU별 데이터 테이블
static mut PERCPU_DATA: [Option<UnsafeCell<PerCpu>>; MAX_CPUS] = {
    const NONE: Option<UnsafeCell<PerCpu>> = None;
    [NONE; MAX_CPUS]
};

/// 현재 CPU 데이터 가져오기
pub fn current_cpu() -> &'static mut PerCpu {
    let cpu_id = get_current_cpu_id();
    unsafe {
        PERCPU_DATA[cpu_id as usize]
            .as_ref()
            .expect("CPU not initialized")
            .get()
            .as_mut()
            .unwrap()
    }
}

/// CPU 데이터 초기화
pub fn init_percpu(cpu_id: u32, apic_id: u8) {
    let percpu = PerCpu {
        cpu_id,
        apic_id,
        current_task: None,
        run_queue: RunQueue::new(),
        idle_task: TaskId(0),
        kernel_stack: allocate_kernel_stack(),
        tss: TaskStateSegment::new(),
        timer_ticks: 0,
        stats: CpuStats::default(),
    };
    
    unsafe {
        PERCPU_DATA[cpu_id as usize] = Some(UnsafeCell::new(percpu));
    }
}
```

### 1.3 멀티코어 스케줄러

```rust
// kernel/src/smp/scheduler.rs

//! 멀티코어 스케줄러
//! 
//! 로드 밸런싱 및 CPU 어피니티 지원

use alloc::collections::VecDeque;
use spin::Mutex;

/// 글로벌 스케줄러
pub struct SmpScheduler {
    /// 전역 태스크 풀 (CPU 미할당)
    global_pool: Mutex<VecDeque<TaskId>>,
    /// CPU 수
    cpu_count: usize,
    /// 로드 밸런싱 주기 (틱)
    balance_interval: u64,
}

impl SmpScheduler {
    pub fn new(cpu_count: usize) -> Self {
        SmpScheduler {
            global_pool: Mutex::new(VecDeque::new()),
            cpu_count,
            balance_interval: 100, // 100 틱마다
        }
    }
    
    /// 태스크 생성 시 CPU 할당
    pub fn assign_cpu(&self, task: &Task) -> u32 {
        match task.cpu_affinity {
            CpuAffinity::Any => self.find_least_loaded_cpu(),
            CpuAffinity::Prefer(cpu_id) => cpu_id,
            CpuAffinity::Pinned(cpu_id) => cpu_id,
        }
    }
    
    /// 가장 여유로운 CPU 찾기
    fn find_least_loaded_cpu(&self) -> u32 {
        let mut min_load = usize::MAX;
        let mut best_cpu = 0;
        
        for i in 0..self.cpu_count {
            let cpu = get_percpu(i as u32);
            let load = cpu.run_queue.len();
            
            if load < min_load {
                min_load = load;
                best_cpu = i as u32;
            }
        }
        
        best_cpu
    }
    
    /// 로드 밸런싱
    pub fn balance(&self) {
        // 각 CPU의 로드 수집
        let mut loads: Vec<(u32, usize)> = (0..self.cpu_count as u32)
            .map(|i| (i, get_percpu(i).run_queue.len()))
            .collect();
        
        // 로드 순 정렬
        loads.sort_by_key(|(_, load)| *load);
        
        // 최대/최소 차이가 임계값 초과 시 마이그레이션
        if loads.last().unwrap().1 - loads.first().unwrap().1 > 2 {
            let (heavy_cpu, _) = loads.last().unwrap();
            let (light_cpu, _) = loads.first().unwrap();
            
            self.migrate_task(*heavy_cpu, *light_cpu);
        }
    }
    
    /// 태스크 마이그레이션
    fn migrate_task(&self, from_cpu: u32, to_cpu: u32) {
        let from_queue = &mut get_percpu(from_cpu).run_queue;
        
        // 마이그레이션 가능한 태스크 찾기
        if let Some(task_id) = from_queue.find_migratable() {
            let task = get_task(task_id);
            
            // 어피니티 확인
            match task.cpu_affinity {
                CpuAffinity::Pinned(_) => return, // 마이그레이션 불가
                _ => {}
            }
            
            // 태스크 이동
            from_queue.remove(task_id);
            get_percpu(to_cpu).run_queue.push(task_id);
            
            log::debug!("Migrated task {} from CPU {} to CPU {}", 
                task_id.0, from_cpu, to_cpu);
        }
    }
}

/// CPU 어피니티
#[derive(Debug, Clone, Copy)]
pub enum CpuAffinity {
    /// 아무 CPU에서나 실행
    Any,
    /// 특정 CPU 선호 (마이그레이션 가능)
    Prefer(u32),
    /// 특정 CPU 고정 (마이그레이션 불가)
    Pinned(u32),
}
```

### 1.4 스핀락 최적화

```rust
// kernel/src/sync/spinlock.rs

//! 멀티코어용 스핀락

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

/// 티켓 스핀락 (공정성 보장)
pub struct TicketSpinLock<T> {
    next_ticket: AtomicU32,
    now_serving: AtomicU32,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for TicketSpinLock<T> {}
unsafe impl<T: Send> Sync for TicketSpinLock<T> {}

impl<T> TicketSpinLock<T> {
    pub const fn new(data: T) -> Self {
        TicketSpinLock {
            next_ticket: AtomicU32::new(0),
            now_serving: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }
    
    pub fn lock(&self) -> TicketSpinLockGuard<T> {
        // 티켓 발급
        let my_ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        
        // 내 차례까지 대기
        while self.now_serving.load(Ordering::Acquire) != my_ticket {
            // 백오프
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }
        
        TicketSpinLockGuard { lock: self }
    }
    
    pub fn try_lock(&self) -> Option<TicketSpinLockGuard<T>> {
        let current = self.now_serving.load(Ordering::Relaxed);
        
        if self.next_ticket
            .compare_exchange(
                current,
                current + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            Some(TicketSpinLockGuard { lock: self })
        } else {
            None
        }
    }
}

pub struct TicketSpinLockGuard<'a, T> {
    lock: &'a TicketSpinLock<T>,
}

impl<'a, T> Deref for TicketSpinLockGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for TicketSpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for TicketSpinLockGuard<'a, T> {
    fn drop(&mut self) {
        // 다음 티켓 처리
        self.lock.now_serving.fetch_add(1, Ordering::Release);
    }
}
```

---

## 2. 성능 최적화

### 2.1 메모리 할당 최적화

```rust
// kernel/src/memory/slab.rs

//! Slab 할당자
//! 
//! 작은 객체의 빠른 할당/해제

use alloc::vec::Vec;
use core::ptr::NonNull;

/// Slab 캐시
pub struct SlabCache {
    /// 객체 크기
    obj_size: usize,
    /// 슬랩당 객체 수
    objs_per_slab: usize,
    /// 부분 채움 슬랩
    partial: Vec<NonNull<Slab>>,
    /// 빈 슬랩
    free: Vec<NonNull<Slab>>,
    /// 꽉 찬 슬랩
    full: Vec<NonNull<Slab>>,
    /// CPU별 캐시
    cpu_cache: [Option<CpuSlabCache>; MAX_CPUS],
}

/// CPU별 슬랩 캐시 (락 없이 할당)
struct CpuSlabCache {
    /// 프리리스트
    freelist: *mut FreeObject,
    /// 현재 슬랩
    slab: Option<NonNull<Slab>>,
}

/// 슬랩
#[repr(C)]
struct Slab {
    /// 다음 슬랩
    next: Option<NonNull<Slab>>,
    /// 프리리스트
    freelist: *mut FreeObject,
    /// 사용 중인 객체 수
    in_use: usize,
    /// 객체 수용량
    capacity: usize,
}

/// 프리리스트 엔트리
#[repr(C)]
struct FreeObject {
    next: *mut FreeObject,
}

impl SlabCache {
    pub fn new(obj_size: usize) -> Self {
        let obj_size = obj_size.max(core::mem::size_of::<FreeObject>());
        let obj_size = (obj_size + 7) & !7; // 8바이트 정렬
        
        let slab_size = 4096; // 4KB 슬랩
        let objs_per_slab = (slab_size - core::mem::size_of::<Slab>()) / obj_size;
        
        SlabCache {
            obj_size,
            objs_per_slab,
            partial: Vec::new(),
            free: Vec::new(),
            full: Vec::new(),
            cpu_cache: [None; MAX_CPUS],
        }
    }
    
    /// 객체 할당 (CPU 캐시 우선)
    pub fn alloc(&mut self) -> Option<NonNull<u8>> {
        let cpu_id = get_current_cpu_id() as usize;
        
        // CPU 캐시에서 시도
        if let Some(ref mut cache) = self.cpu_cache[cpu_id] {
            if !cache.freelist.is_null() {
                unsafe {
                    let obj = cache.freelist;
                    cache.freelist = (*obj).next;
                    return Some(NonNull::new_unchecked(obj as *mut u8));
                }
            }
        }
        
        // 슬랩에서 할당
        self.alloc_slow()
    }
    
    fn alloc_slow(&mut self) -> Option<NonNull<u8>> {
        // partial 슬랩에서 시도
        if let Some(slab) = self.partial.last() {
            let slab = unsafe { slab.as_ref() };
            if !slab.freelist.is_null() {
                unsafe {
                    let obj = slab.freelist;
                    (*slab).freelist = (*obj).next;
                    (*slab).in_use += 1;
                    return Some(NonNull::new_unchecked(obj as *mut u8));
                }
            }
        }
        
        // 새 슬랩 할당
        self.grow_cache()?;
        self.alloc_slow()
    }
    
    fn grow_cache(&mut self) -> Option<()> {
        // 새 슬랩용 페이지 할당
        let page = allocate_pages(1)?;
        
        // 슬랩 초기화
        let slab_ptr = page.as_ptr() as *mut Slab;
        unsafe {
            (*slab_ptr).next = None;
            (*slab_ptr).in_use = 0;
            (*slab_ptr).capacity = self.objs_per_slab;
            
            // 프리리스트 구성
            let objects_start = (slab_ptr as *mut u8)
                .add(core::mem::size_of::<Slab>());
            
            let mut prev: *mut FreeObject = core::ptr::null_mut();
            for i in (0..self.objs_per_slab).rev() {
                let obj = objects_start.add(i * self.obj_size) as *mut FreeObject;
                (*obj).next = prev;
                prev = obj;
            }
            (*slab_ptr).freelist = prev;
        }
        
        self.partial.push(NonNull::new(slab_ptr)?);
        Some(())
    }
    
    /// 객체 해제
    pub fn free(&mut self, ptr: NonNull<u8>) {
        let cpu_id = get_current_cpu_id() as usize;
        
        // CPU 캐시에 반환
        if let Some(ref mut cache) = self.cpu_cache[cpu_id] {
            unsafe {
                let obj = ptr.as_ptr() as *mut FreeObject;
                (*obj).next = cache.freelist;
                cache.freelist = obj;
            }
            return;
        }
        
        // 슬랩으로 반환
        self.free_slow(ptr);
    }
    
    fn free_slow(&mut self, ptr: NonNull<u8>) {
        // 슬랩 찾기 및 반환
        todo!()
    }
}
```

### 2.2 제로 카피 IPC

```rust
// kernel/src/ipc/zerocopy.rs

//! 제로 카피 IPC
//! 
//! 대용량 데이터 전송 시 복사 없이 페이지 매핑만 변경

/// 대용량 버퍼 전송
pub struct SharedBuffer {
    /// 물리 페이지들
    pages: Vec<PhysFrame>,
    /// 크기
    size: usize,
    /// 소유자
    owner: TaskId,
    /// 공유 상대
    shared_with: Option<TaskId>,
}

impl SharedBuffer {
    /// 공유 버퍼 생성
    pub fn new(size: usize) -> Result<Self, IpcError> {
        let page_count = (size + 4095) / 4096;
        let mut pages = Vec::with_capacity(page_count);
        
        for _ in 0..page_count {
            let frame = allocate_frame()?;
            pages.push(frame);
        }
        
        Ok(SharedBuffer {
            pages,
            size,
            owner: current_task_id(),
            shared_with: None,
        })
    }
    
    /// 다른 태스크와 공유 (제로 카피)
    pub fn share_with(&mut self, target: TaskId) -> Result<u64, IpcError> {
        if self.shared_with.is_some() {
            return Err(IpcError::AlreadyShared);
        }
        
        // 대상 태스크의 주소 공간에 매핑
        let target_addr_space = get_task(target).address_space();
        let target_vaddr = target_addr_space.find_free_region(self.size)?;
        
        for (i, frame) in self.pages.iter().enumerate() {
            target_addr_space.map_frame(
                target_vaddr + (i * 4096) as u64,
                *frame,
                PageFlags::PRESENT | PageFlags::USER | PageFlags::NO_EXECUTE,
            )?;
        }
        
        self.shared_with = Some(target);
        Ok(target_vaddr)
    }
    
    /// 공유 해제
    pub fn unshare(&mut self) -> Result<(), IpcError> {
        if let Some(target) = self.shared_with.take() {
            let target_addr_space = get_task(target).address_space();
            
            for (i, _) in self.pages.iter().enumerate() {
                target_addr_space.unmap(/* ... */)?;
            }
        }
        Ok(())
    }
    
    /// 소유권 이전 (이동)
    pub fn transfer(mut self, target: TaskId) -> Result<u64, IpcError> {
        // 원래 소유자에서 언매핑
        let old_addr_space = get_task(self.owner).address_space();
        // ... 언매핑
        
        // 새 소유자에 매핑
        let new_addr_space = get_task(target).address_space();
        let new_vaddr = new_addr_space.find_free_region(self.size)?;
        
        for (i, frame) in self.pages.iter().enumerate() {
            new_addr_space.map_frame(
                new_vaddr + (i * 4096) as u64,
                *frame,
                PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE,
            )?;
        }
        
        self.owner = target;
        Ok(new_vaddr)
    }
}
```

### 2.3 WASM AOT 캐시 관리

RECOMMENDATIONS.md 1.2절 결정에 따른 AOT 재컴파일 메커니즘:

```rust
// runtime/src/aot/cache_manager.rs

//! AOT 컴파일 캐시 관리

use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::PathBuf;

/// AOT 캐시 관리자
pub struct AotCacheManager {
    /// 캐시 디렉토리
    cache_dir: PathBuf,
    /// 모듈 해시 -> 컴파일된 경로 매핑
    cache_index: HashMap<[u8; 32], PathBuf>,
    /// 무효화된 항목
    invalidated: Vec<[u8; 32]>,
}

impl AotCacheManager {
    pub fn new(cache_dir: PathBuf) -> Self {
        let cache_index = Self::load_index(&cache_dir);
        
        AotCacheManager {
            cache_dir,
            cache_index,
            invalidated: Vec::new(),
        }
    }
    
    /// 캐시 조회
    pub fn get(&self, wasm_bytes: &[u8]) -> Option<&PathBuf> {
        let hash = self.compute_hash(wasm_bytes);
        
        if self.invalidated.contains(&hash) {
            return None;
        }
        
        self.cache_index.get(&hash)
    }
    
    /// 캐시 저장
    pub fn store(&mut self, wasm_bytes: &[u8], compiled: &[u8]) -> Result<(), CacheError> {
        let hash = self.compute_hash(wasm_bytes);
        let path = self.cache_dir.join(hex::encode(hash));
        
        std::fs::write(&path, compiled)?;
        self.cache_index.insert(hash, path);
        self.save_index()?;
        
        Ok(())
    }
    
    /// 특정 모듈 무효화 및 재컴파일
    pub fn invalidate_and_recompile(
        &mut self,
        old_wasm: &[u8],
        new_wasm: &[u8],
        engine: &wasmtime::Engine,
    ) -> Result<wasmtime::Module, CacheError> {
        // 이전 캐시 무효화
        let old_hash = self.compute_hash(old_wasm);
        self.invalidated.push(old_hash);
        
        if let Some(path) = self.cache_index.remove(&old_hash) {
            std::fs::remove_file(path).ok();
        }
        
        // 새 버전 컴파일
        let module = wasmtime::Module::new(engine, new_wasm)?;
        
        // 직렬화하여 캐시
        let serialized = module.serialize()?;
        self.store(new_wasm, &serialized)?;
        
        log::info!("Module recompiled and cached");
        Ok(module)
    }
    
    /// 시스템 앱 업데이트 처리
    pub fn handle_system_update(
        &mut self,
        app_name: &str,
        new_wasm: &[u8],
        engine: &wasmtime::Engine,
    ) -> Result<(), CacheError> {
        log::info!("System app '{}' updated, recompiling AOT cache", app_name);
        
        // 해당 앱의 모든 이전 캐시 삭제
        let prefix = format!("system/{}/", app_name);
        self.cache_index.retain(|_, path| {
            !path.to_string_lossy().contains(&prefix)
        });
        
        // 새 버전 컴파일 및 캐시
        let module = wasmtime::Module::new(engine, new_wasm)?;
        let serialized = module.serialize()?;
        self.store(new_wasm, &serialized)?;
        
        Ok(())
    }
    
    /// 전체 캐시 무효화
    pub fn invalidate_all(&mut self) {
        for (hash, _) in self.cache_index.drain() {
            self.invalidated.push(hash);
        }
        
        // 캐시 디렉토리 정리
        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                std::fs::remove_file(entry.path()).ok();
            }
        }
    }
    
    fn compute_hash(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
    
    fn load_index(cache_dir: &PathBuf) -> HashMap<[u8; 32], PathBuf> {
        // 인덱스 파일 로드
        todo!()
    }
    
    fn save_index(&self) -> Result<(), CacheError> {
        // 인덱스 파일 저장
        todo!()
    }
}
```

---

## 3. 윈도우 매니저

### 3.1 데스크탑 환경

```rust
// graphics/src/desktop/mod.rs

//! 데스크탑 환경

/// 데스크탑 매니저
pub struct DesktopManager {
    /// 워크스페이스
    workspaces: Vec<Workspace>,
    /// 현재 워크스페이스
    current_workspace: usize,
    /// 작업표시줄
    taskbar: Taskbar,
    /// 시스템 트레이
    system_tray: SystemTray,
    /// 알림 센터
    notification_center: NotificationCenter,
    /// 앱 런처
    app_launcher: AppLauncher,
}

impl DesktopManager {
    pub fn new() -> Self {
        DesktopManager {
            workspaces: vec![Workspace::new("Workspace 1")],
            current_workspace: 0,
            taskbar: Taskbar::new(),
            system_tray: SystemTray::new(),
            notification_center: NotificationCenter::new(),
            app_launcher: AppLauncher::new(),
        }
    }
    
    /// 워크스페이스 추가
    pub fn add_workspace(&mut self, name: &str) {
        self.workspaces.push(Workspace::new(name));
    }
    
    /// 워크스페이스 전환
    pub fn switch_workspace(&mut self, index: usize) {
        if index < self.workspaces.len() {
            // 현재 워크스페이스 애니메이션 아웃
            self.workspaces[self.current_workspace].animate_out();
            
            self.current_workspace = index;
            
            // 새 워크스페이스 애니메이션 인
            self.workspaces[self.current_workspace].animate_in();
        }
    }
    
    /// 입력 이벤트 처리
    pub fn handle_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::KeyDown { keycode: Keycode::Meta, .. } => {
                self.app_launcher.toggle();
            }
            _ => {
                // 현재 워크스페이스로 전달
                self.workspaces[self.current_workspace].handle_input(event);
            }
        }
    }
    
    /// 프레임 렌더링
    pub fn render(&mut self, scene: &mut Scene) {
        // 배경
        self.render_wallpaper(scene);
        
        // 워크스페이스
        self.workspaces[self.current_workspace].render(scene);
        
        // 작업표시줄
        self.taskbar.render(scene);
        
        // 시스템 트레이
        self.system_tray.render(scene);
        
        // 앱 런처 (열려있을 때)
        if self.app_launcher.is_visible() {
            self.app_launcher.render(scene);
        }
        
        // 알림
        self.notification_center.render(scene);
    }
    
    fn render_wallpaper(&self, scene: &mut Scene) {
        // 배경화면 렌더링
    }
}

/// 워크스페이스
pub struct Workspace {
    name: String,
    windows: Vec<WindowId>,
    layout: LayoutMode,
}

/// 레이아웃 모드
pub enum LayoutMode {
    /// 자유 배치
    Floating,
    /// 타일링
    Tiling(TilingLayout),
}

/// 작업표시줄
pub struct Taskbar {
    position: TaskbarPosition,
    height: u32,
    pinned_apps: Vec<AppId>,
    running_apps: Vec<WindowId>,
}

/// 시스템 트레이
pub struct SystemTray {
    items: Vec<TrayItem>,
    clock: Clock,
    battery: Option<BatteryIndicator>,
    network: NetworkIndicator,
}
```

### 3.2 윈도우 관리

```rust
// graphics/src/desktop/window_manager.rs

//! 윈도우 관리

/// 윈도우 관리자
pub struct WindowManager {
    /// 윈도우 목록
    windows: HashMap<WindowId, ManagedWindow>,
    /// 포커스 순서
    focus_stack: Vec<WindowId>,
    /// 윈도우 스냅 영역
    snap_zones: Vec<SnapZone>,
    /// 애니메이션 상태
    animations: Vec<WindowAnimation>,
}

impl WindowManager {
    pub fn new() -> Self {
        // 스냅 영역 초기화 (화면 분할)
        let snap_zones = vec![
            SnapZone::LeftHalf,
            SnapZone::RightHalf,
            SnapZone::TopLeft,
            SnapZone::TopRight,
            SnapZone::BottomLeft,
            SnapZone::BottomRight,
            SnapZone::Maximize,
        ];
        
        WindowManager {
            windows: HashMap::new(),
            focus_stack: Vec::new(),
            snap_zones,
            animations: Vec::new(),
        }
    }
    
    /// 윈도우 드래그 시작
    pub fn start_drag(&mut self, window: WindowId, start_x: i32, start_y: i32) {
        if let Some(w) = self.windows.get_mut(&window) {
            w.drag_state = Some(DragState {
                start_x,
                start_y,
                original_x: w.x,
                original_y: w.y,
            });
        }
    }
    
    /// 윈도우 드래그
    pub fn drag(&mut self, window: WindowId, current_x: i32, current_y: i32) {
        if let Some(w) = self.windows.get_mut(&window) {
            if let Some(ref drag) = w.drag_state {
                let dx = current_x - drag.start_x;
                let dy = current_y - drag.start_y;
                
                w.x = drag.original_x + dx;
                w.y = drag.original_y + dy;
                
                // 스냅 영역 하이라이트
                self.check_snap_zone(current_x, current_y);
            }
        }
    }
    
    /// 윈도우 드래그 종료
    pub fn end_drag(&mut self, window: WindowId, end_x: i32, end_y: i32) {
        // 스냅 영역에 있으면 스냅
        if let Some(zone) = self.get_snap_zone(end_x, end_y) {
            self.snap_to_zone(window, zone);
        }
        
        if let Some(w) = self.windows.get_mut(&window) {
            w.drag_state = None;
        }
    }
    
    /// 스냅 영역에 윈도우 배치
    fn snap_to_zone(&mut self, window: WindowId, zone: SnapZone) {
        let (x, y, width, height) = match zone {
            SnapZone::LeftHalf => (0, 0, SCREEN_WIDTH / 2, SCREEN_HEIGHT),
            SnapZone::RightHalf => (SCREEN_WIDTH / 2, 0, SCREEN_WIDTH / 2, SCREEN_HEIGHT),
            SnapZone::TopLeft => (0, 0, SCREEN_WIDTH / 2, SCREEN_HEIGHT / 2),
            SnapZone::TopRight => (SCREEN_WIDTH / 2, 0, SCREEN_WIDTH / 2, SCREEN_HEIGHT / 2),
            SnapZone::BottomLeft => (0, SCREEN_HEIGHT / 2, SCREEN_WIDTH / 2, SCREEN_HEIGHT / 2),
            SnapZone::BottomRight => (SCREEN_WIDTH / 2, SCREEN_HEIGHT / 2, SCREEN_WIDTH / 2, SCREEN_HEIGHT / 2),
            SnapZone::Maximize => (0, 0, SCREEN_WIDTH, SCREEN_HEIGHT),
        };
        
        // 애니메이션 추가
        self.animations.push(WindowAnimation::MoveTo {
            window,
            target_x: x as i32,
            target_y: y as i32,
            target_width: width,
            target_height: height,
            duration_ms: 200,
            elapsed_ms: 0,
        });
    }
    
    /// 윈도우 최소화
    pub fn minimize(&mut self, window: WindowId) {
        if let Some(w) = self.windows.get_mut(&window) {
            w.state = WindowState::Minimized;
            
            // 작업표시줄로 축소 애니메이션
            self.animations.push(WindowAnimation::Minimize {
                window,
                duration_ms: 150,
                elapsed_ms: 0,
            });
        }
    }
    
    /// 윈도우 최대화/복원
    pub fn toggle_maximize(&mut self, window: WindowId) {
        if let Some(w) = self.windows.get_mut(&window) {
            match w.state {
                WindowState::Normal => {
                    w.saved_geometry = Some((w.x, w.y, w.width, w.height));
                    w.state = WindowState::Maximized;
                    self.snap_to_zone(window, SnapZone::Maximize);
                }
                WindowState::Maximized => {
                    if let Some((x, y, width, height)) = w.saved_geometry {
                        w.state = WindowState::Normal;
                        self.animations.push(WindowAnimation::MoveTo {
                            window,
                            target_x: x,
                            target_y: y,
                            target_width: width,
                            target_height: height,
                            duration_ms: 200,
                            elapsed_ms: 0,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    
    /// 애니메이션 틱
    pub fn update_animations(&mut self, delta_ms: u32) {
        for anim in self.animations.iter_mut() {
            anim.update(delta_ms);
        }
        
        // 완료된 애니메이션 적용
        self.animations.retain(|anim| !anim.is_complete());
    }
}

/// 스냅 영역
#[derive(Debug, Clone, Copy)]
pub enum SnapZone {
    LeftHalf,
    RightHalf,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Maximize,
}
```

---

## 4. 추가 드라이버

### 4.1 VirtIO-Sound

```rust
// kernel/src/drivers/virtio_sound.rs

//! VirtIO Sound 드라이버

use virtio::VirtQueue;

/// VirtIO Sound 디바이스
pub struct VirtioSound {
    /// 컨트롤 큐
    control_queue: VirtQueue,
    /// 이벤트 큐
    event_queue: VirtQueue,
    /// TX 큐 (재생)
    tx_queue: VirtQueue,
    /// RX 큐 (녹음)
    rx_queue: VirtQueue,
    /// 스트림 정보
    streams: Vec<SoundStream>,
}

/// 오디오 스트림
pub struct SoundStream {
    pub id: u32,
    pub direction: StreamDirection,
    pub channels: u8,
    pub sample_rate: u32,
    pub format: SampleFormat,
    pub buffer: AudioBuffer,
}

#[derive(Debug, Clone, Copy)]
pub enum StreamDirection {
    Output,
    Input,
}

#[derive(Debug, Clone, Copy)]
pub enum SampleFormat {
    U8,
    S16Le,
    S24Le,
    S32Le,
    Float32Le,
}

impl VirtioSound {
    pub fn new(transport: VirtioTransport) -> Result<Self, VirtioError> {
        // 피처 협상
        let features = transport.negotiate_features(
            VIRTIO_SOUND_F_PCM_STREAM | VIRTIO_SOUND_F_PCM_PARAM
        )?;
        
        // 큐 생성
        let control_queue = VirtQueue::new(&transport, 0, 64)?;
        let event_queue = VirtQueue::new(&transport, 1, 64)?;
        let tx_queue = VirtQueue::new(&transport, 2, 256)?;
        let rx_queue = VirtQueue::new(&transport, 3, 256)?;
        
        // 스트림 정보 쿼리
        let streams = Self::query_streams(&control_queue)?;
        
        Ok(VirtioSound {
            control_queue,
            event_queue,
            tx_queue,
            rx_queue,
            streams,
        })
    }
    
    /// 오디오 데이터 전송 (재생)
    pub fn play(&mut self, stream_id: u32, data: &[u8]) -> Result<(), SoundError> {
        let stream = self.streams.iter()
            .find(|s| s.id == stream_id)
            .ok_or(SoundError::InvalidStream)?;
        
        // PCM 데이터를 버퍼에 복사
        let buffer = allocate_dma_buffer(data.len())?;
        buffer.copy_from_slice(data);
        
        // TX 큐에 제출
        let header = VirtioSoundPcmXfer {
            stream_id,
            latency_bytes: 0,
        };
        
        self.tx_queue.add_buffer(&[
            &header.as_bytes(),
            buffer.as_slice(),
        ], false)?;
        
        self.tx_queue.kick()?;
        
        Ok(())
    }
    
    fn query_streams(control_queue: &VirtQueue) -> Result<Vec<SoundStream>, VirtioError> {
        // 스트림 정보 쿼리
        todo!()
    }
}
```

### 4.2 USB (XHCI)

```rust
// kernel/src/drivers/xhci.rs

//! XHCI (USB 3.x) 드라이버

use x86_64::PhysAddr;

/// XHCI 컨트롤러
pub struct XhciController {
    /// MMIO 레지스터
    regs: XhciRegs,
    /// 디바이스 컨텍스트 배열
    device_contexts: DeviceContextArray,
    /// 커맨드 링
    command_ring: CommandRing,
    /// 이벤트 링
    event_ring: EventRing,
    /// 연결된 디바이스
    devices: Vec<UsbDevice>,
}

impl XhciController {
    pub fn new(base_addr: PhysAddr) -> Result<Self, XhciError> {
        // MMIO 매핑
        let regs = unsafe { XhciRegs::from_addr(base_addr) };
        
        // 컨트롤러 초기화 시퀀스
        // 1. 호스트 컨트롤러 리셋
        regs.usbcmd.write(USBCMD_HCRST);
        while regs.usbcmd.read() & USBCMD_HCRST != 0 {}
        
        // 2. 디바이스 컨텍스트 배열 설정
        let device_contexts = DeviceContextArray::new(regs.max_slots())?;
        regs.dcbaap.write(device_contexts.phys_addr());
        
        // 3. 커맨드 링 설정
        let command_ring = CommandRing::new(256)?;
        regs.crcr.write(command_ring.phys_addr() | CRCR_RCS);
        
        // 4. 이벤트 링 설정
        let event_ring = EventRing::new(256)?;
        // 인터럽터 설정...
        
        // 5. 컨트롤러 시작
        regs.usbcmd.write(USBCMD_RS);
        
        Ok(XhciController {
            regs,
            device_contexts,
            command_ring,
            event_ring,
            devices: Vec::new(),
        })
    }
    
    /// 포트 변경 이벤트 처리
    pub fn handle_port_change(&mut self, port: u8) {
        let status = self.regs.port_status(port);
        
        if status & PORTSC_CSC != 0 {
            // 연결 상태 변경
            if status & PORTSC_CCS != 0 {
                self.handle_device_connect(port);
            } else {
                self.handle_device_disconnect(port);
            }
            
            // 상태 플래그 클리어
            self.regs.port_status_mut(port).write(status | PORTSC_CSC);
        }
    }
    
    fn handle_device_connect(&mut self, port: u8) {
        log::info!("USB device connected on port {}", port);
        
        // 포트 리셋
        let status = self.regs.port_status(port);
        self.regs.port_status_mut(port).write(status | PORTSC_PR);
        
        // 리셋 완료 대기...
        
        // 슬롯 할당
        let slot_id = self.enable_slot();
        
        // 디바이스 초기화
        // ...
    }
    
    fn enable_slot(&mut self) -> u8 {
        // Enable Slot 커맨드
        self.command_ring.enqueue(TrbEnableSlot::new());
        self.command_ring.ring_doorbell(&self.regs);
        
        // 완료 이벤트 대기
        let event = self.event_ring.poll();
        event.slot_id
    }
}
```

---

## 5. LVM 지원

RECOMMENDATIONS.md 4.3절 결정에 따라 Phase 4에서 구현:

```rust
// storage/src/lvm/mod.rs

//! Logical Volume Manager

pub mod pv;    // Physical Volume
pub mod vg;    // Volume Group
pub mod lv;    // Logical Volume

/// LVM 관리자
pub struct LvmManager {
    /// 물리 볼륨
    physical_volumes: Vec<PhysicalVolume>,
    /// 볼륨 그룹
    volume_groups: Vec<VolumeGroup>,
}

impl LvmManager {
    pub fn new() -> Self {
        LvmManager {
            physical_volumes: Vec::new(),
            volume_groups: Vec::new(),
        }
    }
    
    /// PV 생성
    pub fn pvcreate(&mut self, device: &str) -> Result<(), LvmError> {
        let pv = PhysicalVolume::create(device)?;
        self.physical_volumes.push(pv);
        Ok(())
    }
    
    /// VG 생성
    pub fn vgcreate(&mut self, name: &str, pv_paths: &[&str]) -> Result<(), LvmError> {
        let pvs: Vec<_> = pv_paths.iter()
            .filter_map(|path| self.find_pv(path))
            .collect();
        
        if pvs.len() != pv_paths.len() {
            return Err(LvmError::PvNotFound);
        }
        
        let vg = VolumeGroup::create(name, pvs)?;
        self.volume_groups.push(vg);
        Ok(())
    }
    
    /// LV 생성
    pub fn lvcreate(&mut self, vg_name: &str, lv_name: &str, size: u64) -> Result<(), LvmError> {
        let vg = self.find_vg_mut(vg_name)
            .ok_or(LvmError::VgNotFound)?;
        
        vg.create_lv(lv_name, size)?;
        Ok(())
    }
    
    /// LV 확장
    pub fn lvextend(&mut self, lv_path: &str, new_size: u64) -> Result<(), LvmError> {
        let (vg_name, lv_name) = parse_lv_path(lv_path)?;
        
        let vg = self.find_vg_mut(vg_name)
            .ok_or(LvmError::VgNotFound)?;
        
        vg.extend_lv(lv_name, new_size)?;
        Ok(())
    }
    
    fn find_pv(&self, path: &str) -> Option<&PhysicalVolume> {
        self.physical_volumes.iter().find(|pv| pv.device_path == path)
    }
    
    fn find_vg_mut(&mut self, name: &str) -> Option<&mut VolumeGroup> {
        self.volume_groups.iter_mut().find(|vg| vg.name == name)
    }
}

/// Physical Volume
pub struct PhysicalVolume {
    /// 디바이스 경로
    device_path: String,
    /// UUID
    uuid: [u8; 16],
    /// 전체 크기
    total_size: u64,
    /// 할당된 크기
    allocated_size: u64,
    /// 익스텐트 크기
    extent_size: u64,
    /// 익스텐트 맵
    extent_map: Vec<ExtentInfo>,
}

/// Volume Group
pub struct VolumeGroup {
    /// 이름
    name: String,
    /// UUID
    uuid: [u8; 16],
    /// 소속 PV
    pvs: Vec<PhysicalVolume>,
    /// 논리 볼륨
    lvs: Vec<LogicalVolume>,
    /// 익스텐트 크기
    extent_size: u64,
}

/// Logical Volume
pub struct LogicalVolume {
    /// 이름
    name: String,
    /// UUID
    uuid: [u8; 16],
    /// 크기
    size: u64,
    /// 세그먼트 (물리 위치 매핑)
    segments: Vec<LvSegment>,
}

impl LogicalVolume {
    /// LV를 블록 디바이스로 제공
    pub fn as_block_device(&self) -> impl BlockDevice {
        LvBlockDevice::new(self)
    }
}
```

---

## 6. 문서화

### 6.1 API 문서

```rust
// 모든 공개 API에 rustdoc 주석 추가 기준

/// WASM 모듈 로딩 및 실행
/// 
/// # Arguments
/// 
/// * `wasm_bytes` - WASM 바이트코드
/// * `config` - 모듈 설정
/// 
/// # Returns
/// 
/// 성공 시 `WasmModule` 인스턴스, 실패 시 `WasmError`
/// 
/// # Examples
/// 
/// ```rust
/// let module = WasmModule::load(
///     include_bytes!("hello.wasm"),
///     ModuleConfig::default(),
/// )?;
/// 
/// let result = module.call("main", &[])?;
/// ```
/// 
/// # Errors
/// 
/// - `WasmError::InvalidModule` - WASM 형식 오류
/// - `WasmError::OutOfMemory` - 메모리 부족
/// - `WasmError::CompilationFailed` - 컴파일 실패
/// 
/// # Panics
/// 
/// 없음
/// 
/// # Safety
/// 
/// 이 함수는 안전합니다. 내부적으로 WASM 샌드박스가 보안을 보장합니다.
pub fn load(wasm_bytes: &[u8], config: ModuleConfig) -> Result<WasmModule, WasmError>;
```

### 6.2 아키텍처 문서

```
docs/
  architecture/
    OVERVIEW.md           - 전체 아키텍처 개요
    KERNEL.md             - 커널 설계
    WASM_RUNTIME.md       - WASM 런타임 설계
    GRAPHICS.md           - 그래픽 스택 설계
    IPC.md                - IPC 시스템 설계
    FILESYSTEM.md         - 파일시스템 설계
    NETWORKING.md         - 네트워킹 설계
    SECURITY.md           - 보안 모델
  api/
    SYSCALL.md            - 시스템 콜 레퍼런스
    WASI.md               - WASI 확장 레퍼런스
    WEBGPU.md             - WebGPU API 레퍼런스
  guides/
    BUILDING.md           - 빌드 가이드
    RUNNING.md            - 실행 가이드
    DEVELOPING_APPS.md    - 앱 개발 가이드
    CONTRIBUTING.md       - 기여 가이드
```

---

## 7. 성능 벤치마크

### 7.1 벤치마크 프레임워크

```rust
// bench/src/lib.rs

//! 성능 벤치마크 프레임워크

/// 벤치마크 결과
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u64,
    pub total_time_ns: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub avg_ns: u64,
    pub std_dev: f64,
}

/// 벤치마크 매크로
#[macro_export]
macro_rules! benchmark {
    ($name:expr, $iterations:expr, $code:block) => {{
        let mut times = Vec::with_capacity($iterations);
        
        // 워밍업
        for _ in 0..10 {
            $code
        }
        
        // 측정
        for _ in 0..$iterations {
            let start = get_timestamp_ns();
            $code
            let end = get_timestamp_ns();
            times.push(end - start);
        }
        
        BenchmarkResult::from_times($name, &times)
    }};
}

impl BenchmarkResult {
    pub fn from_times(name: &str, times: &[u64]) -> Self {
        let total: u64 = times.iter().sum();
        let avg = total / times.len() as u64;
        let min = *times.iter().min().unwrap();
        let max = *times.iter().max().unwrap();
        
        let variance: f64 = times.iter()
            .map(|&t| (t as f64 - avg as f64).powi(2))
            .sum::<f64>() / times.len() as f64;
        let std_dev = variance.sqrt();
        
        BenchmarkResult {
            name: name.to_string(),
            iterations: times.len() as u64,
            total_time_ns: total,
            min_ns: min,
            max_ns: max,
            avg_ns: avg,
            std_dev,
        }
    }
    
    pub fn print(&self) {
        println!("Benchmark: {}", self.name);
        println!("  Iterations: {}", self.iterations);
        println!("  Total:  {} ns", self.total_time_ns);
        println!("  Min:    {} ns", self.min_ns);
        println!("  Max:    {} ns", self.max_ns);
        println!("  Avg:    {} ns", self.avg_ns);
        println!("  StdDev: {:.2} ns", self.std_dev);
        println!();
    }
}
```

### 7.2 핵심 벤치마크

| 벤치마크 | 목표 | 측정 방법 |
|---------|------|----------|
| WASM 함수 호출 | < 100ns | 빈 WASM 함수 호출 오버헤드 |
| IPC 메시지 왕복 | < 10us | 작은 메시지 송수신 |
| 컨텍스트 스위치 | < 5us | 태스크 전환 시간 |
| 파일 읽기 4KB | < 50us | 캐시된 파일 읽기 |
| 메모리 할당 4KB | < 1us | 슬랩 할당자 성능 |
| GPU 프레임 제출 | < 1ms | wgpu 커맨드 제출 |

---

## 8. 검증 체크리스트

Phase 4 완료 전 확인 사항:

- [ ] 4개 이상 CPU에서 SMP 부팅
- [ ] 멀티코어 로드 밸런싱 동작
- [ ] 슬랩 할당자 벤치마크 통과
- [ ] 제로 카피 IPC 동작
- [ ] AOT 캐시 재컴파일 동작
- [ ] 데스크탑 환경 UI
- [ ] 윈도우 스냅 기능
- [ ] VirtIO-Sound 재생
- [ ] USB 디바이스 인식 (XHCI)
- [ ] LVM 논리 볼륨 생성/마운트
- [ ] API 문서 완성도 80% 이상
- [ ] 핵심 벤치마크 목표 달성

---

## 9. 릴리스 준비

### 9.1 빌드 자동화

```yaml
# .github/workflows/release.yml

name: Release Build

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install dependencies
        run: |
          rustup target add x86_64-unknown-none
          cargo install bootimage
      
      - name: Build kernel
        run: cargo build --release
      
      - name: Create disk image
        run: cargo bootimage --release
      
      - name: Create release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            target/x86_64-unknown-none/release/bootimage-kernel.bin
            docs/RELEASE_NOTES.md
```

### 9.2 릴리스 체크리스트

- [ ] 버전 번호 업데이트
- [ ] CHANGELOG.md 작성
- [ ] 릴리스 노트 작성
- [ ] 문서 최종 검토
- [ ] 테스트 통과 확인
- [ ] 벤치마크 결과 기록
- [ ] 디스크 이미지 생성
- [ ] 릴리스 태그 생성
