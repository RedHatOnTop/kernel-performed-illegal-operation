# 설계 변경 및 개선 제안 사항

## 개요

본 문서는 "kernel-performed-illegal-operation" (KPIO) 프로젝트의 기획서를 검토한 결과, 기술적 실현 가능성, 성능 최적화, 보안 강화, 그리고 개발 효율성 측면에서 제안하는 변경 사항을 정리합니다.

---

## 1. 아키텍처 관련 제안

### 1.1 순수 마이크로커널 아키텍처 (확정)

**현재 설계**: 하이브리드 마이크로커널 (Vulkan 드라이버가 커널 공간에서 실행)

**결정**: 순수 마이크로커널 아키텍처 채택 (GPU 드라이버 사용자 공간 격리 필수)

**결정 근거**:
- Mesa 3D 드라이버는 수백만 줄의 복잡한 코드베이스로 버그가 없기는 사실상 불가능
- 커널 공간에서 드라이버 버그 발생 시 전체 시스템 크래시로 직결
- 안정성을 위해 성능 오버헤드(5-15%)를 감수하는 것이 합리적
- 드라이버 격리로 핫 리로드, 독립적 업데이트 가능

**아키텍처 결정**:

```
[GPU 드라이버 격리 아키텍처 - 필수 적용]

+------------------+
| WASM 애플리케이션 |
+--------+---------+
         |
         | wgpu/Vulkan API (IPC)
         v
+--------+---------+
|  GPU 드라이버    |  <- 사용자 공간 프로세스
|  (Mesa 3D)       |     (크래시 시 자동 재시작)
+--------+---------+
         |
         | 커널 GPU HAL (최소 인터페이스)
         v
+--------+---------+
|   커널 (MMIO/DMA)|  <- 하드웨어 접근만 중개
+------------------+
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | DMA 최적화로 오버헤드 최소화 (목표: 10% 이내) | IPC 레이턴시 불가피 |
| 호환성 | 드라이버 독립 업데이트, 핫스왑 지원 | 커널-드라이버 인터페이스 설계 필요 |
| 안정성 | **드라이버 버그가 시스템 크래시로 이어지지 않음** | 드라이버 재시작 시 GPU 상태 복구 필요 |

---

### 1.2 WASM 실행 모델 개선

**현재 설계**: Cranelift JIT 컴파일러 사용

**제안**: AOT(Ahead-of-Time) 컴파일 옵션 추가

```
[실행 모델 제안]

1. 시스템 애플리케이션 -> AOT 컴파일 (부팅 시 로드)
2. 사용자 애플리케이션 -> JIT 컴파일 (동적 로드)
3. 일회성 스크립트 -> 인터프리터 모드 (Wasmtime baseline)
```

**구현 방안**:
- 설치 시점에 시스템 앱을 네이티브 코드로 AOT 컴파일
- 캐시된 컴파일 결과를 저장하여 재시작 시 재사용
- Wasmtime의 `serialize`/`deserialize` 기능 활용

**AOT 재컴파일 메커니즘** (시스템 앱 업데이트/버그 수정용):

```rust
/// AOT 캐시 관리자
pub struct AotCacheManager {
    cache_dir: PathBuf,
    /// WASM 모듈 해시 -> AOT 바이너리 매핑
    cache_index: HashMap<ModuleHash, AotEntry>,
}

impl AotCacheManager {
    /// 시스템 앱 업데이트 시 호출
    pub fn invalidate_and_recompile(&mut self, module_path: &Path) -> Result<()> {
        let hash = compute_module_hash(module_path)?;
        
        // 1. 기존 AOT 캐시 무효화
        if let Some(entry) = self.cache_index.remove(&hash) {
            fs::remove_file(&entry.aot_path)?;
        }
        
        // 2. 새 AOT 바이너리 생성
        let engine = Engine::new(&aot_config())?;
        let module = Module::from_file(&engine, module_path)?;
        let serialized = module.serialize()?;
        
        // 3. 캐시에 저장
        let aot_path = self.cache_dir.join(format!("{}.aot", hash));
        fs::write(&aot_path, &serialized)?;
        
        self.cache_index.insert(hash, AotEntry { aot_path, version: VERSION });
        Ok(())
    }
    
    /// 부팅 시 무결성 검증
    pub fn verify_cache_integrity(&self) -> Result<Vec<ModuleHash>> {
        let mut invalid = Vec::new();
        for (hash, entry) in &self.cache_index {
            if !self.verify_aot_binary(entry) {
                invalid.push(*hash);
            }
        }
        Ok(invalid)
    }
}
```

**기대 효과**:
- 부팅 시간 단축 (JIT 웜업 제거)
- 메모리 사용량 감소 (컴파일러 메타데이터 불필요)
- 시스템 앱의 예측 가능한 성능
- **시스템 앱 업데이트 시 자동 AOT 재컴파일**

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | AOT: 즉시 네이티브 속도, JIT 웜업 없음 | AOT 컴파일 시간 증가, 디스크 공간 추가 필요 |
| 호환성 | 3가지 모드로 다양한 사용 사례 지원 | AOT 바이너리는 CPU 아키텍처별로 별도 생성 필요 |
| 안정성 | AOT는 런타임 컴파일 오류 제거 | 캐시 무효화 로직 복잡, 버전 불일치 위험 |

---

### 1.3 메모리 관리 개선

**현재 설계**: Buddy Allocator + Slab Allocator

**제안**: SLUB 알고리즘 기반 할당자 도입

**이유**:
- SLAB보다 메모리 단편화 감소
- CPU별 캐시로 멀티코어 확장성 향상
- Linux 커널에서 검증된 알고리즘

**추가 제안**: WASM 샌드박스 전용 메모리 풀

```rust
// 제안하는 구조
pub struct WasmMemoryPool {
    /// 4KB 페이지 풀 (작은 힙용)
    small_pool: PagePool<4096>,
    /// 2MB 대형 페이지 풀 (큰 힙용)
    large_pool: HugePagePool<2097152>,
    /// 인스턴스별 할당 추적
    allocations: BTreeMap<InstanceId, AllocationInfo>,
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | SLUB: CPU 캐시 친화적, 멀티코어 확장성 우수 | SLAB 대비 구현 복잡도 증가 |
| 호환성 | Linux 커널에서 검증된 알고리즘 | 기존 Buddy/Slab 코드 재작성 필요 |
| 안정성 | 단편화 감소로 장기 운영 안정성 향상 | 새 알고리즘 도입에 따른 초기 버그 위험 |

---

## 2. 그래픽 서브시스템 제안

### 2.1 Vulkan 전용 정책의 위험성

**현재 설계**: Vulkan만 지원, OpenGL/DirectX 미지원

**잠재적 문제**:
1. 레거시 애플리케이션 호환성 완전 포기
2. 일부 하드웨어(구형 GPU)에서 Vulkan 미지원
3. 개발자 진입 장벽 증가

**제안**: 계층화된 그래픽 API 지원

```
[제안하는 그래픽 스택]

Layer 3: wgpu (높은 수준 추상화, WASM 앱 기본 API)
    |
Layer 2: Vulkan (네이티브 성능 필요 시)
    |
Layer 1: Mesa 3D Drivers (RADV/ANV/NVK)
    |
Layer 0: GPU Hardware
```

**wgpu 기본 API로 제안하는 이유**:
- WebGPU 표준 기반으로 크로스 플랫폼 호환
- Vulkan보다 사용하기 쉬움
- 내부적으로 Vulkan 백엔드 사용 (성능 손실 미미)
- WASM 앱에 자연스러운 API

**WASM/웹앱 통합 그래픽 계층 설계**:

OS 수준에서 WASM 및 웹앱을 네이티브로 지원하기 위한 그래픽 스택:

```
[통합 그래픽 계층 아키텍처]

┌─────────────────────────────────────────────────────────┐
│                    애플리케이션 계층                      │
├─────────────────┬─────────────────┬─────────────────────┤
│   WASM 네이티브  │    웹앱 (PWA)    │   Rust 네이티브    │
│   애플리케이션   │  WebView 컨테이너 │   애플리케이션     │
└────────┬────────┴────────┬────────┴──────────┬──────────┘
         │                 │                   │
         v                 v                   v
┌─────────────────────────────────────────────────────────┐
│              통합 그래픽 API (kpio-graphics)             │
├─────────────────────────────────────────────────────────┤
│  WebGPU 호환 API  │  Canvas 2D API  │  SVG 렌더링 API   │
│   (wgpu 기반)     │  (Vello 기반)   │  (Vello 기반)     │
└────────┬──────────┴────────┬────────┴──────────┬────────┘
         │                   │                   │
         v                   v                   v
┌─────────────────────────────────────────────────────────┐
│                  Vello 렌더링 엔진                       │
│        (GPU 컴퓨트 기반 2D 벡터 렌더링)                  │
└────────────────────────┬────────────────────────────────┘
                         │
                         v
┌─────────────────────────────────────────────────────────┐
│                    wgpu 추상화 계층                      │
│              (WebGPU 표준 구현체)                        │
└────────────────────────┬────────────────────────────────┘
                         │
                         v
┌─────────────────────────────────────────────────────────┐
│              Vulkan (Mesa 3D - 사용자 공간)              │
└────────────────────────┬────────────────────────────────┘
                         │
                         v
┌─────────────────────────────────────────────────────────┐
│                    GPU 하드웨어                          │
└─────────────────────────────────────────────────────────┘
```

**웹앱 지원을 위한 핵심 컴포넌트**:

```rust
/// 웹앱 그래픽 컨텍스트
pub struct WebAppGraphicsContext {
    /// WebGPU 디바이스 (3D 렌더링용)
    gpu_device: wgpu::Device,
    /// Canvas 2D 컨텍스트 (2D 렌더링용)
    canvas_2d: Canvas2DContext,
    /// SVG 렌더러
    svg_renderer: SvgRenderer,
    /// 컴포지터 서피스
    surface: CompositorSurface,
}

/// Canvas 2D API (HTML5 Canvas 호환)
pub struct Canvas2DContext {
    scene: vello::Scene,
    current_path: Path,
    fill_style: FillStyle,
    stroke_style: StrokeStyle,
    transform: Affine,
}

impl Canvas2DContext {
    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64);
    pub fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64);
    pub fn draw_image(&mut self, image: &Image, dx: f64, dy: f64);
    pub fn fill_text(&mut self, text: &str, x: f64, y: f64);
    // ... HTML5 Canvas API 전체 구현
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | wgpu 오버헤드 미미 (2-5%), 필요시 Vulkan 직접 접근 가능 | 최고 성능을 위해서는 Vulkan 직접 사용 필요 |
| 호환성 | WebGPU 표준으로 웹/네이티브 코드 공유 가능 | OpenGL 레거시 앱은 재작성 필요 |
| 안정성 | wgpu가 Vulkan 복잡성 추상화, 에러 처리 용이 | wgpu 버전 업데이트 시 API 변경 가능성 |

---

### 2.2 Vello 단일 렌더러 + 최소 폴백 (확정)

**현재 설계**: Vello를 컴포지터 렌더러로 사용

**우려 사항**:
- Vello는 아직 pre-1.0 상태로 API 안정성 보장 없음
- GPU 컴퓨트 쉐이더 필수 (구형 하드웨어 제외)
- 이중 렌더러 번갈아 사용 시 추가 안정성 문제 발생 가능

**결정**: Vello 단일 렌더러 + 최소 기능 폴백

듀얼 렌더러 전환은 상태 관리, 시각적 일관성 문제를 야기할 수 있어 단일 렌더러 전략 채택:

```rust
/// 렌더러 설정 (부팅 시 결정, 런타임 전환 없음)
pub enum RendererMode {
    /// GPU 컴퓨트 지원 시 (기본)
    Vello,
    /// GPU 컴퓨트 미지원 시 (구형 하드웨어)
    BasicFramebuffer,
}

/// 부팅 시 렌더러 선택
pub fn select_renderer(gpu_caps: &GpuCapabilities) -> RendererMode {
    if gpu_caps.supports_compute_shaders() {
        RendererMode::Vello
    } else {
        log::warn!("GPU 컴퓨트 미지원: 기본 프레임버퍼 모드로 전환");
        RendererMode::BasicFramebuffer
    }
}

/// 기본 프레임버퍼 렌더러 (최소 기능)
pub struct BasicFramebufferRenderer {
    framebuffer: &'static mut [u32],
    width: u32,
    height: u32,
}

impl BasicFramebufferRenderer {
    /// 단색 사각형만 지원 (텍스트는 비트맵 폰트)
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32);
    pub fn draw_bitmap_char(&mut self, x: u32, y: u32, ch: char);
}
```

**전략 요약**:
- **주 렌더러**: Vello (GPU 컴퓨트 기반, 모든 기능 지원)
- **폴백**: 기본 프레임버퍼 (최소 기능, 콘솔 수준 UI만)
- **런타임 전환 없음**: 부팅 시 결정 후 고정

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 단일 렌더러로 최적화 집중 | GPU 컴퓨트 미지원 시 기능 제한 |
| 호환성 | 최소 폴백으로 구형 하드웨어 부팅 가능 | 폴백 모드에서 GUI 앱 실행 불가 |
| 안정성 | **렌더러 전환 없어 상태 관리 단순화** | Vello 버그 시 폴백으로 전환 불가 |

---

### 2.3 GPU 드라이버 격리

**현재 설계**: Mesa 드라이버가 커널 공간에서 실행

**제안**: GPU 드라이버를 사용자 공간 서비스로 실행

```
[드라이버 격리 아키텍처]

+------------------+
| WASM 애플리케이션 |
+--------+---------+
         |
         | Vulkan IPC
         v
+--------+---------+
|  GPU 드라이버    |  <- 사용자 공간 (권한 분리)
|  (Mesa 3D)       |
+--------+---------+
         |
         | MMIO/DMA (커널 중개)
         v
+--------+---------+
|   커널 GPU HAL   |  <- 최소한의 커널 코드
+------------------+
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | DMA 최적화로 오버헤드 최소화 가능 | IPC 오버헤드 5-15%, 레이턴시 증가 |
| 호환성 | 드라이버 독립적 업데이트, 핫스왑 가능 | 커널-드라이버 인터페이스 설계 복잡 |
| 안정성 | 드라이버 크래시 격리, 재시작 가능 | IPC 데드락 가능성, 복구 로직 필요 |

---

## 3. 네트워킹 서브시스템 제안

### 3.1 smoltcp 한계점

**현재 설계**: smoltcp를 유일한 TCP/IP 스택으로 사용

**우려 사항**:
- 고성능 네트워킹에 부적합 (10Gbps+ 환경)
- 고급 기능 부재 (TCP BBR, MPTCP 등)
- 대규모 연결 처리 한계

**제안**: 모듈식 네트워크 스택

```rust
pub trait NetworkStack: Send + Sync {
    fn create_socket(&mut self, domain: Domain, socket_type: SocketType) 
        -> Result<SocketHandle, NetworkError>;
    fn bind(&mut self, socket: SocketHandle, addr: SocketAddr) 
        -> Result<(), NetworkError>;
    // ... 기타 POSIX 소켓 인터페이스
}

// 구현체
pub struct SmoltcpStack { /* 기본 구현 */ }
pub struct HighPerfStack { /* 고성능 구현, 향후 */ }
```

**결정**: smoltcp 단일 스택 + 선택적 고성능 확장

대부분의 사용자는 고성능 네트워킹이 필요하지 않으므로 smoltcp를 기본으로 하고,
고성능이 필요한 경우에만 선택적으로 확장 스택을 로드:

```rust
/// 네트워크 스택 설정
pub enum NetworkStackConfig {
    /// 기본: smoltcp (대부분의 사용 사례)
    Standard,
    /// 고성능: 선택적 로드 (10Gbps+ 환경)
    HighPerformance {
        /// 사용할 고성능 드라이버
        driver: HighPerfDriver,
    },
}

/// 고성능 드라이버 (선택적 기능 플래그로 빌드)
#[cfg(feature = "high-perf-net")]
pub enum HighPerfDriver {
    /// DPDK 기반 (특정 NIC 최적화)
    Dpdk { pci_addr: PciAddress },
    /// io_uring 기반 (범용)
    IoUring,
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 기본 사용자는 smoltcp로 충분, 필요시 확장 | smoltcp 기본 성능은 1Gbps 수준 |
| 호환성 | 표준 POSIX 소켓 인터페이스 유지 | 고성능 스택은 추가 설정 필요 |
| 안정성 | **smoltcp 단일 스택으로 복잡도 감소** | 고성능 스택은 별도 테스트 필요 |

---

### 3.2 DPDK/XDP 고려

**제안**: 향후 고성능 네트워킹을 위한 인터페이스 예약

```rust
/// 고성능 패킷 처리 인터페이스 (향후 구현)
pub trait PacketProcessor {
    /// 배치 수신
    fn recv_batch(&mut self, batch: &mut [PacketBuf]) -> usize;
    /// 배치 송신
    fn send_batch(&mut self, batch: &[PacketBuf]) -> usize;
    /// 제로카피 버퍼 획득
    fn get_buffer(&mut self) -> Option<PacketBuf>;
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 10-100Gbps 처리 가능, 제로카피로 CPU 부하 감소 | 커널 바이패스로 방화벽/모니터링 우회 |
| 호환성 | 인터페이스 예약으로 향후 확장 용이 | DPDK는 특정 NIC에만 최적화됨 |
| 안정성 | 사용자 공간 처리로 커널 영향 없음 | 사용자 공간 드라이버 크래시 시 패킷 유실 |

---

## 4. 스토리지 서브시스템 제안

### 4.1 FUSE 프로토콜 오버헤드

**현재 설계**: 모든 파일시스템을 FUSE로 구현

**우려 사항**:
- FUSE IPC 오버헤드가 상당함 (특히 메타데이터 작업)
- 작은 파일 I/O에서 성능 저하 심각

**제안**: 커널 내장 파일시스템 + FUSE 확장

```
[계층화된 파일시스템 지원]

계층 1: 커널 내장 (ext4, FAT32) - 성능 크리티컬
계층 2: FUSE 커널측 (WASM 드라이버용 인터페이스)
계층 3: WASM 파일시스템 (사용자 정의 FS)
```

**구현 방안**:
- ext4, FAT32는 커널에 내장하여 최대 성능 확보
- NTFS, Btrfs 등은 WASM FUSE 드라이버로 구현
- 캐싱 레이어를 통해 FUSE 오버헤드 완화

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 내장 FS는 FUSE 대비 5-10배 빠름 | 커널 코드 증가로 빌드/테스트 시간 증가 |
| 호환성 | ext4/FAT32로 대부분 사용 사례 커버 | 새 FS 추가 시 커널 수정 또는 FUSE 사용 결정 필요 |
| 안정성 | 내장 FS 버그는 커널 크래시 유발 가능 | FUSE FS 버그는 해당 마운트만 영향 |

---

### 4.2 블록 캐시 개선

**현재 설계**: 단순 LRU 캐시

**제안**: ARC(Adaptive Replacement Cache) 알고리즘 도입

```rust
/// ARC 캐시 구현 (향후)
pub struct AdaptiveCache {
    /// 최근 접근 리스트 (LRU)
    t1: LruList<BlockEntry>,
    /// 빈번 접근 리스트 (LFU)
    t2: LruList<BlockEntry>,
    /// T1 유령 엔트리
    b1: LruList<BlockKey>,
    /// T2 유령 엔트리
    b2: LruList<BlockKey>,
    /// 적응 파라미터
    p: usize,
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | LRU 대비 20-30% 높은 히트율, 자동 적응 | 메타데이터 오버헤드 (4개 리스트 유지) |
| 호환성 | ZFS/PostgreSQL에서 검증된 알고리즘 | 기존 LRU 캐시 코드 전면 재작성 |
| 안정성 | 스캔 저항성으로 예측 가능한 성능 | 적응 파라미터 튜닝 필요, 초기 학습 기간 존재 |

---

### 4.3 파티션 테이블 지원 확장

**현재 설계**: GPT, MBR 지원

**결정**: LVM 지원 (Phase 2), 소프트웨어 RAID (Phase 4 이후 검토)

**구현 우선순위**:

| 우선순위 | 기능 | 단계 | 이유 |
|----------|------|------|------|
| 1 | GPT/MBR | Phase 0 | 기본 부팅 필수 |
| 2 | LVM | Phase 2 | 서버 환경 필수, 유연한 볼륨 관리 |
| 3 | 소프트웨어 RAID | Phase 4+ | 복잡도 높음, 하드웨어 RAID로 대체 가능 |

```rust
/// LVM 지원 (Phase 2)
pub mod lvm {
    pub struct PhysicalVolume { /* PV 메타데이터 */ }
    pub struct VolumeGroup { /* VG 메타데이터 */ }
    pub struct LogicalVolume { /* LV 메타데이터 */ }
    
    /// Linux LVM2 메타데이터 호환
    pub fn parse_lvm_metadata(pv: &BlockDevice) -> Result<VolumeGroup>;
}

/// 소프트웨어 RAID (Phase 4+ 검토)
#[cfg(feature = "software-raid")]
pub mod raid {
    // 향후 구현 예정
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | LVM 스냅샷으로 백업 효율화 | LVM 메타데이터 오버헤드 미미 |
| 호환성 | Linux LVM2 메타데이터 100% 호환 목표 | 소프트웨어 RAID는 후순위 |
| 안정성 | LVM으로 볼륨 관리 유연성 확보 | 메타데이터 손상 대비 백업 필요 |

---

## 5. 보안 관련 제안

### 5.1 Capability 시스템 강화

**현재 설계**: 기본적인 권한 비트 기반 Capability

**제안**: 세분화된 Capability 계층

```rust
/// 확장된 Capability 모델
#[derive(Debug, Clone)]
pub struct Capability {
    /// 대상 리소스
    pub resource: ResourceId,
    /// 허용된 작업
    pub operations: Operations,
    /// 시간 제한 (옵션)
    pub expiry: Option<Timestamp>,
    /// 위임 가능 여부
    pub delegatable: bool,
    /// 취소 토큰
    pub revocation_token: RevocationToken,
    /// 감사 정책
    pub audit_policy: AuditPolicy,
}

/// 리소스 유형별 Capability
pub enum ResourceCapability {
    File(FileCapability),
    Network(NetworkCapability),
    Device(DeviceCapability),
    Memory(MemoryCapability),
    Process(ProcessCapability),
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 비트 연산으로 빠른 권한 검사 | 세분화된 검사로 약간의 오버헤드 |
| 호환성 | POSIX 권한 모델과 매핑 가능 | 기존 Unix 권한과 개념적 차이로 학습 필요 |
| 안정성 | 시간 제한/취소로 권한 누출 방지 | Capability 전파 추적 복잡, 디버깅 어려움 |

---

### 5.2 WASM 샌드박스 강화

**현재 설계**: Wasmtime 기본 샌드박스

**제안**: 다중 방어 계층

```
[다중 보안 계층]

Layer 4: 리소스 쿼터 (CPU, 메모리, I/O 제한)
Layer 3: Capability 검증 (모든 시스템 콜)
Layer 2: WASM 메모리 격리 (선형 메모리 경계)
Layer 1: 페이지 테이블 격리 (하드웨어 지원)
Layer 0: IOMMU/VT-d (DMA 공격 방지)
```

**추가 제안**:
- CFI(Control Flow Integrity) 활성화
- ASLR 적용 (WASM 인스턴스 주소 무작위화)
- Spectre/Meltdown 완화 (선택적, 성능 민감 환경에서 비활성화 가능)

**성능 영향 분석**:

| 보안 계층 | 성능 오버헤드 | 비고 |
|-----------|--------------|------|
| Layer 4: 리소스 쿼터 | ~1% | 카운터 증가만 |
| Layer 3: Capability 검증 | ~2% | 비트 연산 기반 |
| Layer 2: WASM 메모리 격리 | 0% | WASM 기본 동작 |
| Layer 1: 페이지 테이블 격리 | 0% | 하드웨어 지원 |
| Layer 0: IOMMU | ~1% | DMA 매핑 오버헤드 |
| **총합 (Spectre 완화 제외)** | **~4%** | 수용 가능 |
| Spectre 완화 (선택적) | 10-30% | 보안 요구사항에 따라 선택 |

**Spectre 완화 설정**:

```rust
/// 보안 프로파일
pub enum SecurityProfile {
    /// 최대 보안 (Spectre 완화 포함)
    Maximum,
    /// 균형 (기본값, Spectre 완화 제외)
    Balanced,
    /// 성능 우선 (일부 검사 간소화)
    Performance,
}
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | **기본 설정 ~4% 오버헤드로 수용 가능** | Spectre 완화 시 추가 10-30% |
| 호환성 | WASI 표준으로 앱 이식성 보장 | 일부 시스템 기능 접근 제한 |
| 안정성 | 5중 방어로 단일 취약점 익스플로잇 어려움 | 보안 프로파일 선택 필요 |

---

### 5.3 시큐어 부트 통합

**현재 설계**: UEFI 부팅만 명시

**제안**: 전체 신뢰 체인 구축

```
[부트 신뢰 체인]

1. UEFI Secure Boot (펌웨어 검증)
       |
2. 부트로더 서명 검증
       |
3. 커널 이미지 서명 검증
       |
4. 초기 램디스크 검증
       |
5. 시스템 서비스 WASM 모듈 검증
       |
6. 사용자 애플리케이션 런타임 검증
```

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 부팅 시 한 번만 검증, 런타임 영향 없음 | 서명 검증으로 부팅 시간 약간 증가 |
| 호환성 | UEFI 표준으로 대부분 하드웨어 지원 | 자체 서명 인증서 관리 필요, 키 배포 복잡 |
| 안정성 | 부트킷/루트킷 공격 원천 차단 | 키 분실 시 부팅 불가, 복구 절차 복잡 |

---

## 6. 개발 및 테스트 관련 제안

### 6.1 단위 테스트 프레임워크

**제안**: `#[no_std]` 환경 전용 테스트 프레임워크

```rust
/// 커널 테스트 매크로
#[macro_export]
macro_rules! kernel_test {
    ($name:ident, $body:block) => {
        #[test_case]
        fn $name() {
            serial_print!("test {} ... ", stringify!($name));
            $body
            serial_println!("[ok]");
        }
    };
}

/// 테스트 러너
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}
```

---

### 6.2 에뮬레이션 환경 개선

**현재 설계**: QEMU 기반 테스트

**제안**: 다중 에뮬레이션 백엔드 지원

| 백엔드 | 용도 | 장점 |
|--------|------|------|
| QEMU | 기본 개발/테스트 | 빠른 부팅, 디버깅 용이 |
| Bochs | 정밀 검증 | 하드웨어 정확도 높음 |
| Cloud Hypervisor | 성능 테스트 | 실제 환경에 가까움 |
| 실제 하드웨어 | 최종 검증 | 실제 동작 확인 |

---

### 6.3 문서화 자동화

**제안**: 아키텍처 다이어그램 자동 생성

```yaml
# .github/workflows/docs.yml
- name: Generate Architecture Diagrams
  run: |
    cargo doc --document-private-items
    d2 docs/diagrams/*.d2 docs/images/
    mdbook build docs/
```

---

## 7. 로드맵 조정 제안

### 7.1 현재 로드맵 분석

**우려 사항**:
- Phase 1에 너무 많은 핵심 기능 포함
- 병렬 개발 가능한 컴포넌트 구분 부족
- 의존성 순서가 명확하지 않음

### 7.2 수정된 로드맵 제안 (단계 중심)

AI 기반 개발 환경에서는 기간보다 **단계별 의존성과 완료 조건**이 중요합니다.

```
[수정된 개발 로드맵 - 단계 중심]

Phase 0: 기초 (Foundation)
├── 선행 조건: 없음
├── 완료 조건: QEMU에서 "Hello, Kernel" 출력
├── 작업 항목:
│   ├── 부팅 가능한 커널 스켈레톤
│   ├── 시리얼 콘솔 출력
│   ├── 기본 메모리 관리 (페이지 할당)
│   └── 단위 테스트 인프라
└── 병렬 가능: 문서화, CI/CD 파이프라인

Phase 1: 코어 (Core)
├── 선행 조건: Phase 0 완료
├── 완료 조건: WASM "Hello World" 앱 실행
├── 작업 항목:
│   ├── 인터럽트/예외 처리
│   ├── 스케줄러 (단일 코어)
│   ├── Wasmtime 통합
│   ├── WASI 기본 구현 (fd_write, clock_time_get)
│   └── VirtIO-Blk 드라이버
└── 병렬 가능: VirtIO-Net (Phase 2 준비)

Phase 2: 사용자 공간 (Userspace)
├── 선행 조건: Phase 1 완료
├── 완료 조건: WASM 쉘에서 파일 시스템 탐색 가능
├── 작업 항목:
│   ├── IPC 시스템 (메시지 전달)
│   ├── VFS 및 ext4 파일시스템
│   ├── TCP/IP 네트워킹 (smoltcp)
│   └── 기본 쉘 (WASM)
└── 병렬 가능: FAT32, 추가 WASI 함수

Phase 3: 그래픽 (Graphics)
├── 선행 조건: Phase 2 완료 (파일시스템, IPC 필요)
├── 완료 조건: wgpu 삼각형 렌더링
├── 작업 항목:
│   ├── Vulkan 드라이버 통합 (Mesa)
│   ├── wgpu 백엔드 구현
│   ├── 컴포지터 기초 (단일 윈도우)
│   └── 키보드/마우스 입력
└── 병렬 가능: 폰트 렌더링, 오디오 (선택)

Phase 4: 완성 (Polish)
├── 선행 조건: Phase 3 완료
├── 완료 조건: 데모 애플리케이션 실행
├── 작업 항목:
│   ├── 멀티코어 지원 (SMP)
│   ├── 성능 최적화 (프로파일링 기반)
│   ├── 윈도우 매니저 완성
│   └── 문서화 완료
└── 병렬 가능: 추가 드라이버, 유틸리티
```

### 7.3 단계별 게이트 기준

| 단계 | 게이트 테스트 | 통과 기준 |
|------|--------------|----------|
| Phase 0 | 부팅 테스트 | 5초 내 시리얼 출력 |
| Phase 1 | WASM 실행 테스트 | "Hello World" 출력 |
| Phase 2 | 파일 I/O 테스트 | 파일 읽기/쓰기 성공 |
| Phase 3 | 렌더링 테스트 | 프레임버퍼에 도형 표시 |
| Phase 4 | 통합 테스트 | 데모 앱 10분 안정 실행 |

---

## 8. 종속성 관련 제안

### 8.1 Wasmtime 버전 고정

**현재**: Wasmtime 17.0

**제안**: 보안 패치만 적용하는 보수적 업데이트 정책

**근거**:
- WASM 런타임은 보안 크리티컬 컴포넌트
- 메이저 버전 업데이트는 충분한 테스트 후 적용
- LTS 브랜치 추적 권장

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 버전 고정으로 예측 가능한 성능 | 신규 최적화 혜택 지연 |
| 호환성 | API 안정성 보장, 앱 호환성 유지 | 새 WASM 기능 지원 지연 |
| 안정성 | 테스트된 버전만 사용, 회귀 방지 | 보안 패치 백포트 관리 부담 |

---

### 8.2 Mesa 3D 버전 관리

**우려 사항**:
- Mesa는 빠르게 변화하는 프로젝트
- Vulkan 드라이버 API가 자주 변경됨

**제안**: 
- 특정 Mesa 버전에 고정 (예: 23.3.x)
- 드라이버 인터페이스 추상화 레이어 도입
- 버전별 테스트 매트릭스 유지

**장단점 분석**:

| 측면 | 장점 | 단점 |
|------|------|------|
| 성능 | 버전 고정으로 최적화 튜닝 가능 | 새 GPU 지원 및 드라이버 최적화 지연 |
| 호환성 | 추상화 레이어로 Mesa 교체 용이 | 추상화로 인한 기능 제한 가능성 |
| 안정성 | 테스트된 조합만 배포 | Mesa 보안 패치 추적 필요 |

---

## 9. 결론

위 제안 사항들은 우선순위에 따라 다음과 같이 분류됩니다:

### 즉시 적용 권장
1. AOT 컴파일 옵션 추가 (성능)
2. wgpu를 기본 그래픽 API로 채택 (개발 편의성)
3. 커널 내장 파일시스템 유지 (성능)
4. 테스트 프레임워크 구축 (품질)

### 중기 검토 대상
1. GPU 드라이버 사용자 공간 분리 (안정성)
2. ARC 캐시 알고리즘 (성능)
3. 확장된 Capability 모델 (보안)

### 장기 고려 사항
1. 고성능 네트워크 스택 (확장성)
2. LVM/RAID 지원 (서버 환경)
3. 시큐어 부트 통합 (보안)

이 제안들은 프로젝트의 성공적인 완료를 위한 기술적 권고사항이며, 실제 적용 여부는 팀의 리소스와 우선순위에 따라 결정해야 합니다.
