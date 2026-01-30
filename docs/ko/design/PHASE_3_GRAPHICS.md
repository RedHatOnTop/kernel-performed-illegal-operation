# Phase 3: 그래픽 (Graphics) 설계 문서

## 개요

Phase 3는 KPIO 운영체제에 그래픽 사용자 인터페이스를 추가하는 단계입니다. GPU 드라이버 격리 아키텍처, wgpu 기반 통합 그래픽 API, Vello 렌더러, 그리고 기본 컴포지터를 구현합니다.

---

## 선행 조건

- Phase 2 완료 (IPC, 파일시스템, 네트워킹)

## 완료 조건

- wgpu 삼각형 렌더링
- 기본 윈도우 생성
- 키보드/마우스 입력 처리

---

## 1. GPU 드라이버 격리 아키텍처

RECOMMENDATIONS.md 1.1절 결정에 따라, GPU 드라이버는 **사용자 공간 프로세스**로 실행됩니다.

### 1.1 커널 GPU HAL

```rust
// kernel/src/gpu/hal.rs

//! GPU 하드웨어 추상화 레이어 (최소 구현)
//! 
//! 커널은 MMIO/DMA 접근만 중개하고, 실제 드라이버 로직은 사용자 공간에서 실행

use x86_64::PhysAddr;

/// GPU 디바이스 정보
#[derive(Debug)]
pub struct GpuDevice {
    /// PCI 버스/디바이스/펑션
    pub bdf: (u8, u8, u8),
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// BAR 주소
    pub bars: [Option<BarInfo>; 6],
}

/// BAR 정보
#[derive(Debug, Clone)]
pub struct BarInfo {
    pub base: PhysAddr,
    pub size: u64,
    pub is_memory: bool,
    pub is_64bit: bool,
    pub prefetchable: bool,
}

/// GPU HAL 오퍼레이션
pub trait GpuHal: Send + Sync {
    /// MMIO 영역 매핑 요청
    fn map_mmio(&self, bar: u8, offset: u64, size: u64) -> Result<MmioMapping, GpuError>;
    
    /// DMA 버퍼 할당
    fn allocate_dma_buffer(&self, size: u64, alignment: u64) -> Result<DmaBuffer, GpuError>;
    
    /// DMA 버퍼 해제
    fn free_dma_buffer(&self, buffer: DmaBuffer) -> Result<(), GpuError>;
    
    /// MSI-X 인터럽트 등록
    fn register_interrupt(&self, vector: u16, handler: IpcPort) -> Result<(), GpuError>;
    
    /// 인터럽트 해제
    fn unregister_interrupt(&self, vector: u16) -> Result<(), GpuError>;
}

/// MMIO 매핑
#[derive(Debug)]
pub struct MmioMapping {
    /// 가상 주소 (사용자 공간)
    pub virt_addr: u64,
    /// 크기
    pub size: u64,
}

/// DMA 버퍼
#[derive(Debug)]
pub struct DmaBuffer {
    /// 가상 주소 (커널 + 사용자 공간 공유)
    pub virt_addr: u64,
    /// 물리 주소 (GPU용)
    pub phys_addr: PhysAddr,
    /// 크기
    pub size: u64,
}

#[derive(Debug)]
pub enum GpuError {
    DeviceNotFound,
    InvalidBar,
    OutOfMemory,
    PermissionDenied,
    AlreadyMapped,
}
```

### 1.2 GPU 드라이버 서비스 (사용자 공간)

```rust
// gpu-driver/src/main.rs (사용자 공간 WASM 또는 네이티브)

//! GPU 드라이버 서비스
//! 
//! Mesa 3D를 래핑하여 Vulkan API 제공

use ipc::{Port, Message};

/// GPU 드라이버 서비스 메인
fn main() {
    println!("GPU Driver Service starting...");
    
    // 1. 커널에서 GPU HAL 접근 권한 획득
    let gpu_hal = acquire_gpu_hal();
    
    // 2. Mesa 3D 드라이버 초기화
    let mesa_driver = init_mesa_driver(&gpu_hal);
    
    // 3. Vulkan 디바이스 생성
    let vk_device = mesa_driver.create_vulkan_device();
    
    // 4. IPC 서비스 포트 생성
    let service_port = Port::create("gpu.vulkan");
    
    println!("GPU Driver ready, listening on gpu.vulkan");
    
    // 5. 메시지 루프
    loop {
        let msg = service_port.receive();
        
        match msg.msg_type {
            GpuMessageType::CreateSurface => handle_create_surface(&vk_device, msg),
            GpuMessageType::CreateSwapchain => handle_create_swapchain(&vk_device, msg),
            GpuMessageType::SubmitCommands => handle_submit_commands(&vk_device, msg),
            GpuMessageType::Present => handle_present(&vk_device, msg),
            GpuMessageType::Shutdown => break,
            _ => msg.reply_error(GpuError::InvalidRequest),
        }
    }
    
    println!("GPU Driver shutting down");
}

/// GPU 메시지 타입
#[repr(u32)]
enum GpuMessageType {
    CreateSurface = 1,
    DestroySurface = 2,
    CreateSwapchain = 3,
    DestroySwapchain = 4,
    AcquireImage = 5,
    SubmitCommands = 6,
    Present = 7,
    CreateBuffer = 8,
    DestroyBuffer = 9,
    CreateImage = 10,
    DestroyImage = 11,
    Shutdown = 0xFFFF,
}
```

### 1.3 GPU 드라이버 크래시 복구

```rust
// kernel/src/gpu/recovery.rs

//! GPU 드라이버 크래시 자동 복구

use alloc::sync::Arc;
use spin::Mutex;

/// GPU 드라이버 상태 감시
pub struct GpuDriverMonitor {
    /// 드라이버 프로세스 ID
    driver_pid: Option<TaskId>,
    /// 마지막 하트비트
    last_heartbeat: u64,
    /// 재시작 횟수
    restart_count: u32,
    /// 최대 재시작 횟수
    max_restarts: u32,
}

impl GpuDriverMonitor {
    pub fn new() -> Self {
        GpuDriverMonitor {
            driver_pid: None,
            last_heartbeat: 0,
            restart_count: 0,
            max_restarts: 5,
        }
    }
    
    /// 드라이버 상태 확인 (타이머에서 주기적 호출)
    pub fn check_health(&mut self) {
        let current_time = get_monotonic_time();
        
        if let Some(pid) = self.driver_pid {
            // 프로세스 상태 확인
            if !is_process_alive(pid) {
                log::warn!("GPU driver crashed, attempting restart");
                self.restart_driver();
                return;
            }
            
            // 하트비트 타임아웃 확인 (5초)
            if current_time - self.last_heartbeat > 5_000_000_000 {
                log::warn!("GPU driver heartbeat timeout");
                self.kill_and_restart();
            }
        }
    }
    
    /// 드라이버 재시작
    fn restart_driver(&mut self) {
        if self.restart_count >= self.max_restarts {
            log::error!("GPU driver exceeded max restart attempts, entering fallback mode");
            enter_framebuffer_fallback();
            return;
        }
        
        self.restart_count += 1;
        log::info!("Restarting GPU driver (attempt {})", self.restart_count);
        
        // GPU 하드웨어 리셋
        reset_gpu_hardware();
        
        // 새 드라이버 프로세스 시작
        match spawn_gpu_driver() {
            Ok(pid) => {
                self.driver_pid = Some(pid);
                self.last_heartbeat = get_monotonic_time();
            }
            Err(e) => {
                log::error!("Failed to spawn GPU driver: {:?}", e);
                enter_framebuffer_fallback();
            }
        }
    }
    
    fn kill_and_restart(&mut self) {
        if let Some(pid) = self.driver_pid {
            kill_process(pid);
        }
        self.restart_driver();
    }
    
    /// 하트비트 수신
    pub fn heartbeat(&mut self, pid: TaskId) {
        if self.driver_pid == Some(pid) {
            self.last_heartbeat = get_monotonic_time();
        }
    }
}

/// 프레임버퍼 폴백 모드 진입
fn enter_framebuffer_fallback() {
    log::warn!("Entering basic framebuffer fallback mode");
    // Vello 대신 기본 프레임버퍼 렌더러 사용
    COMPOSITOR.lock().set_renderer(BasicFramebufferRenderer::new());
}
```

---

## 2. wgpu 통합 그래픽 API

### 2.1 wgpu 백엔드 (Vulkan)

```rust
// graphics/src/wgpu_backend.rs

//! wgpu Vulkan 백엔드 구현
//! 
//! 사용자 공간 GPU 드라이버와 IPC로 통신

use wgpu_core::api;
use wgpu_hal::vulkan;

/// KPIO wgpu 백엔드
pub struct KpioWgpuBackend {
    /// GPU 드라이버 IPC 포트
    gpu_port: IpcPort,
    /// Vulkan 인스턴스
    instance: vulkan::Instance,
    /// 어댑터 목록
    adapters: Vec<vulkan::Adapter>,
}

impl KpioWgpuBackend {
    pub fn new() -> Result<Self, BackendError> {
        // GPU 드라이버 서비스 연결
        let gpu_port = IpcPort::connect("gpu.vulkan")?;
        
        // Vulkan 인스턴스 생성 (드라이버에서)
        let create_msg = Message::new(GpuMessageType::CreateInstance);
        gpu_port.send(create_msg)?;
        
        let reply = gpu_port.receive()?;
        let instance_handle = reply.parse_handle()?;
        
        // 어댑터 열거
        let adapters = Self::enumerate_adapters(&gpu_port)?;
        
        Ok(KpioWgpuBackend {
            gpu_port,
            instance: vulkan::Instance::from_handle(instance_handle),
            adapters,
        })
    }
    
    fn enumerate_adapters(port: &IpcPort) -> Result<Vec<vulkan::Adapter>, BackendError> {
        let msg = Message::new(GpuMessageType::EnumerateAdapters);
        port.send(msg)?;
        
        let reply = port.receive()?;
        // 어댑터 정보 파싱
        todo!()
    }
}
```

### 2.2 WebGPU 호환 API (WASM용)

```rust
// graphics/src/webgpu_api.rs

//! WASM 앱용 WebGPU 호환 API
//! 
//! WASI 확장으로 노출

use wasmtime::*;

/// WebGPU WASI 확장 등록
pub fn add_webgpu_to_linker(linker: &mut Linker<WasiCtx>) -> Result<()> {
    // Navigator.gpu.requestAdapter()
    linker.func_wrap("webgpu", "request_adapter", request_adapter)?;
    
    // GPUAdapter.requestDevice()
    linker.func_wrap("webgpu", "request_device", request_device)?;
    
    // GPUDevice.createBuffer()
    linker.func_wrap("webgpu", "create_buffer", create_buffer)?;
    
    // GPUDevice.createShaderModule()
    linker.func_wrap("webgpu", "create_shader_module", create_shader_module)?;
    
    // GPUDevice.createRenderPipeline()
    linker.func_wrap("webgpu", "create_render_pipeline", create_render_pipeline)?;
    
    // GPUDevice.createCommandEncoder()
    linker.func_wrap("webgpu", "create_command_encoder", create_command_encoder)?;
    
    // GPUQueue.submit()
    linker.func_wrap("webgpu", "queue_submit", queue_submit)?;
    
    // GPUCanvasContext
    linker.func_wrap("webgpu", "configure_context", configure_context)?;
    linker.func_wrap("webgpu", "get_current_texture", get_current_texture)?;
    
    Ok(())
}

/// request_adapter 구현
fn request_adapter(
    mut caller: Caller<'_, WasiCtx>,
    options_ptr: i32,
    adapter_ptr: i32,
) -> i32 {
    // 어댑터 요청
    let adapter = match WGPU_BACKEND.lock().request_adapter() {
        Ok(a) => a,
        Err(_) => return -1,
    };
    
    // 핸들 저장 및 반환
    let handle = caller.data_mut().store_gpu_resource(adapter);
    
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .unwrap();
    
    memory.data_mut(&mut caller)[adapter_ptr as usize..adapter_ptr as usize + 4]
        .copy_from_slice(&handle.to_le_bytes());
    
    0
}
```

### 2.3 Canvas 2D API (웹앱 호환)

```rust
// graphics/src/canvas2d.rs

//! HTML5 Canvas 2D API 호환 구현
//! 
//! Vello 기반 벡터 렌더링

use vello::{Scene, SceneBuilder, kurbo};
use kurbo::{Affine, Point, Rect};

/// Canvas 2D 컨텍스트
pub struct Canvas2DContext {
    /// Vello 씬
    scene: Scene,
    /// 현재 변환 행렬
    transform: Affine,
    /// 변환 스택
    transform_stack: Vec<Affine>,
    /// 현재 경로
    current_path: kurbo::BezPath,
    /// 채우기 스타일
    fill_style: FillStyle,
    /// 선 스타일
    stroke_style: StrokeStyle,
    /// 선 너비
    line_width: f64,
    /// 폰트
    font: String,
    /// 전역 알파
    global_alpha: f64,
}

impl Canvas2DContext {
    pub fn new(width: u32, height: u32) -> Self {
        Canvas2DContext {
            scene: Scene::new(),
            transform: Affine::IDENTITY,
            transform_stack: Vec::new(),
            current_path: kurbo::BezPath::new(),
            fill_style: FillStyle::Color(Color::BLACK),
            stroke_style: StrokeStyle::Color(Color::BLACK),
            line_width: 1.0,
            font: "16px sans-serif".into(),
            global_alpha: 1.0,
        }
    }
    
    // === 상태 관리 ===
    
    pub fn save(&mut self) {
        self.transform_stack.push(self.transform);
    }
    
    pub fn restore(&mut self) {
        if let Some(t) = self.transform_stack.pop() {
            self.transform = t;
        }
    }
    
    // === 변환 ===
    
    pub fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_translate((x, y).into());
    }
    
    pub fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.then_rotate(angle);
    }
    
    pub fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_scale_non_uniform(x, y);
    }
    
    // === 경로 ===
    
    pub fn begin_path(&mut self) {
        self.current_path = kurbo::BezPath::new();
    }
    
    pub fn move_to(&mut self, x: f64, y: f64) {
        self.current_path.move_to(Point::new(x, y));
    }
    
    pub fn line_to(&mut self, x: f64, y: f64) {
        self.current_path.line_to(Point::new(x, y));
    }
    
    pub fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        self.current_path.curve_to(
            Point::new(cp1x, cp1y),
            Point::new(cp2x, cp2y),
            Point::new(x, y),
        );
    }
    
    pub fn close_path(&mut self) {
        self.current_path.close_path();
    }
    
    // === 그리기 ===
    
    pub fn fill(&mut self) {
        let mut builder = SceneBuilder::for_scene(&mut self.scene);
        let brush = self.fill_style.to_brush();
        builder.fill(
            vello::peniko::Fill::NonZero,
            self.transform,
            &brush,
            None,
            &self.current_path,
        );
    }
    
    pub fn stroke(&mut self) {
        let mut builder = SceneBuilder::for_scene(&mut self.scene);
        let brush = self.stroke_style.to_brush();
        let stroke = kurbo::Stroke::new(self.line_width);
        builder.stroke(
            &stroke,
            self.transform,
            &brush,
            None,
            &self.current_path,
        );
    }
    
    pub fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let rect = Rect::new(x, y, x + width, y + height);
        let mut builder = SceneBuilder::for_scene(&mut self.scene);
        let brush = self.fill_style.to_brush();
        builder.fill(
            vello::peniko::Fill::NonZero,
            self.transform,
            &brush,
            None,
            &rect,
        );
    }
    
    pub fn stroke_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let rect = Rect::new(x, y, x + width, y + height);
        let mut builder = SceneBuilder::for_scene(&mut self.scene);
        let brush = self.stroke_style.to_brush();
        let stroke = kurbo::Stroke::new(self.line_width);
        builder.stroke(
            &stroke,
            self.transform,
            &brush,
            None,
            &rect,
        );
    }
    
    pub fn clear_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        // 투명으로 채우기
        let rect = Rect::new(x, y, x + width, y + height);
        let mut builder = SceneBuilder::for_scene(&mut self.scene);
        builder.fill(
            vello::peniko::Fill::NonZero,
            self.transform,
            &vello::peniko::Color::TRANSPARENT,
            None,
            &rect,
        );
    }
    
    // === 텍스트 ===
    
    pub fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        // Vello 텍스트 렌더링
        // 폰트 파싱 및 글리프 렌더링
        todo!()
    }
    
    // === 이미지 ===
    
    pub fn draw_image(&mut self, image: &Image, dx: f64, dy: f64) {
        // 이미지 블릿
        todo!()
    }
    
    /// 씬을 렌더 타겟에 렌더링
    pub fn flush(&mut self, target: &mut RenderTarget) {
        VELLO_RENDERER.lock().render(&self.scene, target);
        self.scene = Scene::new();
    }
}

/// 채우기 스타일
pub enum FillStyle {
    Color(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    Pattern(Pattern),
}

/// 선 스타일
pub enum StrokeStyle {
    Color(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    Pattern(Pattern),
}
```

---

## 3. Vello 렌더러

### 3.1 Vello 초기화

```rust
// graphics/src/vello_renderer.rs

//! Vello GPU 렌더러 초기화 및 관리

use vello::{Renderer, RendererOptions, Scene};
use vello::util::RenderContext;

/// Vello 렌더러 래퍼
pub struct VelloRenderer {
    /// wgpu 디바이스
    device: wgpu::Device,
    /// wgpu 큐
    queue: wgpu::Queue,
    /// Vello 렌더러
    renderer: Renderer,
    /// 렌더 컨텍스트
    context: RenderContext,
}

impl VelloRenderer {
    pub async fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        // wgpu 초기화
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RenderError::NoAdapter)?;
        
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("KPIO Vello Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await?;
        
        // Vello 렌더러 생성
        let renderer = Renderer::new(
            &device,
            RendererOptions {
                surface_format: Some(wgpu::TextureFormat::Bgra8Unorm),
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: None,
            },
        )?;
        
        let context = RenderContext::new(&device);
        
        Ok(VelloRenderer {
            device,
            queue,
            renderer,
            context,
        })
    }
    
    /// 씬 렌더링
    pub fn render(&mut self, scene: &Scene, target: &wgpu::TextureView, width: u32, height: u32) {
        let render_params = vello::RenderParams {
            base_color: vello::peniko::Color::WHITE,
            width,
            height,
            antialiasing_method: vello::AaConfig::Msaa16,
        };
        
        self.renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                target,
                &render_params,
            )
            .expect("Vello render failed");
    }
}

#[derive(Debug)]
pub enum RenderError {
    NoAdapter,
    DeviceError(wgpu::RequestDeviceError),
    VelloError(vello::Error),
}
```

### 3.2 기본 프레임버퍼 폴백

RECOMMENDATIONS.md 2.2절 결정에 따라, GPU 컴퓨트 미지원 시 사용하는 최소 렌더러:

```rust
// graphics/src/framebuffer_renderer.rs

//! 기본 프레임버퍼 렌더러 (폴백)
//! 
//! GPU 컴퓨트 미지원 하드웨어용 최소 기능 구현

/// 기본 프레임버퍼 렌더러
pub struct BasicFramebufferRenderer {
    /// 프레임버퍼 주소
    framebuffer: &'static mut [u32],
    /// 너비
    width: u32,
    /// 높이
    height: u32,
    /// 비트맵 폰트 (8x16)
    bitmap_font: &'static [u8; 256 * 16],
}

impl BasicFramebufferRenderer {
    /// GOP 프레임버퍼로 초기화
    pub unsafe fn new(
        framebuffer_addr: u64,
        width: u32,
        height: u32,
        stride: u32,
    ) -> Self {
        let fb_size = (stride * height) as usize;
        let framebuffer = core::slice::from_raw_parts_mut(
            framebuffer_addr as *mut u32,
            fb_size,
        );
        
        BasicFramebufferRenderer {
            framebuffer,
            width,
            height,
            bitmap_font: include_bytes!("../assets/font8x16.bin"),
        }
    }
    
    /// 화면 지우기
    pub fn clear(&mut self, color: u32) {
        for pixel in self.framebuffer.iter_mut() {
            *pixel = color;
        }
    }
    
    /// 픽셀 설정
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            self.framebuffer[idx] = color;
        }
    }
    
    /// 사각형 채우기
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, color);
            }
        }
    }
    
    /// 비트맵 문자 그리기
    pub fn draw_char(&mut self, x: u32, y: u32, ch: char, fg: u32, bg: u32) {
        let idx = ch as usize;
        if idx >= 256 {
            return;
        }
        
        for row in 0..16 {
            let bits = self.bitmap_font[idx * 16 + row];
            for col in 0..8 {
                let color = if (bits >> (7 - col)) & 1 == 1 { fg } else { bg };
                self.set_pixel(x + col, y + row as u32, color);
            }
        }
    }
    
    /// 문자열 그리기
    pub fn draw_string(&mut self, x: u32, y: u32, s: &str, fg: u32, bg: u32) {
        let mut cx = x;
        for ch in s.chars() {
            if ch == '\n' {
                // 줄바꿈 처리
                continue;
            }
            self.draw_char(cx, y, ch, fg, bg);
            cx += 8;
        }
    }
    
    /// 수평선 그리기
    pub fn draw_hline(&mut self, x: u32, y: u32, len: u32, color: u32) {
        for dx in 0..len {
            self.set_pixel(x + dx, y, color);
        }
    }
    
    /// 수직선 그리기
    pub fn draw_vline(&mut self, x: u32, y: u32, len: u32, color: u32) {
        for dy in 0..len {
            self.set_pixel(x, y + dy, color);
        }
    }
    
    /// 테두리 사각형
    pub fn draw_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        self.draw_hline(x, y, w, color);
        self.draw_hline(x, y + h - 1, w, color);
        self.draw_vline(x, y, h, color);
        self.draw_vline(x + w - 1, y, h, color);
    }
}
```

---

## 4. 컴포지터

### 4.1 서피스 및 윈도우 관리

```rust
// graphics/src/compositor/surface.rs

//! 컴포지터 서피스 및 윈도우 관리

use alloc::collections::BTreeMap;
use alloc::string::String;

/// 서피스 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceId(pub u64);

/// 서피스
pub struct Surface {
    pub id: SurfaceId,
    /// 소유 태스크
    pub owner: TaskId,
    /// 위치
    pub x: i32,
    pub y: i32,
    /// 크기
    pub width: u32,
    pub height: u32,
    /// 텍스처 (wgpu)
    pub texture: wgpu::Texture,
    /// 가시성
    pub visible: bool,
    /// Z 순서
    pub z_order: i32,
}

/// 윈도우 (서피스 + 데코레이션)
pub struct Window {
    /// 기본 서피스
    pub surface: SurfaceId,
    /// 제목
    pub title: String,
    /// 윈도우 상태
    pub state: WindowState,
    /// 데코레이션 포함 위치
    pub frame_x: i32,
    pub frame_y: i32,
    /// 데코레이션 포함 크기
    pub frame_width: u32,
    pub frame_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

/// 컴포지터
pub struct Compositor {
    /// 모든 서피스
    surfaces: BTreeMap<SurfaceId, Surface>,
    /// 모든 윈도우
    windows: BTreeMap<SurfaceId, Window>,
    /// 다음 서피스 ID
    next_surface_id: u64,
    /// 렌더러
    renderer: Box<dyn CompositorRenderer>,
    /// 화면 크기
    screen_width: u32,
    screen_height: u32,
    /// 포커스된 윈도우
    focused: Option<SurfaceId>,
}

impl Compositor {
    pub fn new(renderer: Box<dyn CompositorRenderer>, width: u32, height: u32) -> Self {
        Compositor {
            surfaces: BTreeMap::new(),
            windows: BTreeMap::new(),
            next_surface_id: 1,
            renderer,
            screen_width: width,
            screen_height: height,
            focused: None,
        }
    }
    
    /// 서피스 생성
    pub fn create_surface(&mut self, owner: TaskId, width: u32, height: u32) -> SurfaceId {
        let id = SurfaceId(self.next_surface_id);
        self.next_surface_id += 1;
        
        let texture = self.renderer.create_texture(width, height);
        
        let surface = Surface {
            id,
            owner,
            x: 100,
            y: 100,
            width,
            height,
            texture,
            visible: true,
            z_order: self.surfaces.len() as i32,
        };
        
        self.surfaces.insert(id, surface);
        id
    }
    
    /// 윈도우 생성 (데코레이션 포함)
    pub fn create_window(&mut self, owner: TaskId, title: String, width: u32, height: u32) -> SurfaceId {
        let surface_id = self.create_surface(owner, width, height);
        
        // 타이틀바 높이 + 테두리
        let frame_height = height + 30 + 4;
        let frame_width = width + 4;
        
        let window = Window {
            surface: surface_id,
            title,
            state: WindowState::Normal,
            frame_x: 100,
            frame_y: 100,
            frame_width,
            frame_height,
        };
        
        self.windows.insert(surface_id, window);
        surface_id
    }
    
    /// 서피스 파괴
    pub fn destroy_surface(&mut self, id: SurfaceId) {
        self.surfaces.remove(&id);
        self.windows.remove(&id);
        
        if self.focused == Some(id) {
            self.focused = None;
        }
    }
    
    /// 프레임 렌더링
    pub fn render_frame(&mut self, target: &wgpu::TextureView) {
        // 배경 렌더링
        self.renderer.clear(target, Color::from_rgb(0x30, 0x30, 0x30));
        
        // Z 순서로 정렬
        let mut surfaces: Vec<_> = self.surfaces.values().collect();
        surfaces.sort_by_key(|s| s.z_order);
        
        // 각 서피스 렌더링
        for surface in surfaces {
            if !surface.visible {
                continue;
            }
            
            // 윈도우 데코레이션 렌더링
            if let Some(window) = self.windows.get(&surface.id) {
                self.render_window_decoration(target, window, surface.id == self.focused.unwrap_or(SurfaceId(0)));
            }
            
            // 서피스 내용 렌더링
            self.renderer.blit(target, &surface.texture, surface.x, surface.y);
        }
        
        // 커서 렌더링
        self.render_cursor(target);
    }
    
    fn render_window_decoration(&mut self, target: &wgpu::TextureView, window: &Window, focused: bool) {
        // 타이틀바 색상
        let title_color = if focused {
            Color::from_rgb(0x00, 0x78, 0xD4) // Windows 11 스타일 파란색
        } else {
            Color::from_rgb(0x60, 0x60, 0x60)
        };
        
        // 타이틀바 렌더링
        // 닫기/최소화/최대화 버튼 렌더링
        // ...
    }
    
    fn render_cursor(&mut self, target: &wgpu::TextureView) {
        // 마우스 커서 렌더링
    }
}
```

### 4.2 입력 처리

```rust
// graphics/src/compositor/input.rs

//! 입력 이벤트 처리

/// 입력 이벤트
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// 키보드 키 눌림
    KeyDown {
        scancode: u8,
        keycode: Keycode,
        modifiers: Modifiers,
    },
    /// 키보드 키 놓임
    KeyUp {
        scancode: u8,
        keycode: Keycode,
        modifiers: Modifiers,
    },
    /// 마우스 이동
    MouseMove {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
    },
    /// 마우스 버튼 눌림
    MouseButtonDown {
        button: MouseButton,
        x: i32,
        y: i32,
    },
    /// 마우스 버튼 놓임
    MouseButtonUp {
        button: MouseButton,
        x: i32,
        y: i32,
    },
    /// 마우스 휠
    MouseWheel {
        delta_x: i32,
        delta_y: i32,
        x: i32,
        y: i32,
    },
}

/// 키코드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keycode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape, Tab, CapsLock, Shift, Control, Alt, Meta,
    Space, Enter, Backspace, Delete, Insert, Home, End,
    PageUp, PageDown, Left, Right, Up, Down,
    Unknown(u8),
}

/// 수정자 키
bitflags::bitflags! {
    pub struct Modifiers: u8 {
        const SHIFT = 1 << 0;
        const CONTROL = 1 << 1;
        const ALT = 1 << 2;
        const META = 1 << 3;
        const CAPS_LOCK = 1 << 4;
        const NUM_LOCK = 1 << 5;
    }
}

/// 마우스 버튼
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

/// 입력 관리자
pub struct InputManager {
    /// 마우스 위치
    mouse_x: i32,
    mouse_y: i32,
    /// 마우스 버튼 상태
    mouse_buttons: u8,
    /// 수정자 키 상태
    modifiers: Modifiers,
    /// 이벤트 큐
    event_queue: VecDeque<InputEvent>,
}

impl InputManager {
    pub fn new() -> Self {
        InputManager {
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons: 0,
            modifiers: Modifiers::empty(),
            event_queue: VecDeque::new(),
        }
    }
    
    /// PS/2 키보드 인터럽트 처리
    pub fn handle_keyboard_interrupt(&mut self, scancode: u8) {
        let is_release = scancode & 0x80 != 0;
        let scancode = scancode & 0x7F;
        
        let keycode = scancode_to_keycode(scancode);
        
        // 수정자 키 업데이트
        match keycode {
            Keycode::Shift => {
                if is_release {
                    self.modifiers.remove(Modifiers::SHIFT);
                } else {
                    self.modifiers.insert(Modifiers::SHIFT);
                }
            }
            Keycode::Control => {
                if is_release {
                    self.modifiers.remove(Modifiers::CONTROL);
                } else {
                    self.modifiers.insert(Modifiers::CONTROL);
                }
            }
            Keycode::Alt => {
                if is_release {
                    self.modifiers.remove(Modifiers::ALT);
                } else {
                    self.modifiers.insert(Modifiers::ALT);
                }
            }
            _ => {}
        }
        
        let event = if is_release {
            InputEvent::KeyUp {
                scancode,
                keycode,
                modifiers: self.modifiers,
            }
        } else {
            InputEvent::KeyDown {
                scancode,
                keycode,
                modifiers: self.modifiers,
            }
        };
        
        self.event_queue.push_back(event);
    }
    
    /// PS/2 마우스 인터럽트 처리
    pub fn handle_mouse_interrupt(&mut self, packet: [u8; 3]) {
        let buttons = packet[0] & 0x07;
        let dx = packet[1] as i8 as i32;
        let dy = -(packet[2] as i8 as i32);
        
        self.mouse_x = (self.mouse_x + dx).clamp(0, SCREEN_WIDTH as i32 - 1);
        self.mouse_y = (self.mouse_y + dy).clamp(0, SCREEN_HEIGHT as i32 - 1);
        
        // 이동 이벤트
        if dx != 0 || dy != 0 {
            self.event_queue.push_back(InputEvent::MouseMove {
                x: self.mouse_x,
                y: self.mouse_y,
                dx,
                dy,
            });
        }
        
        // 버튼 상태 변화 감지
        let old_buttons = self.mouse_buttons;
        self.mouse_buttons = buttons;
        
        for i in 0..3 {
            let mask = 1 << i;
            let button = match i {
                0 => MouseButton::Left,
                1 => MouseButton::Right,
                2 => MouseButton::Middle,
                _ => continue,
            };
            
            if (buttons & mask) != 0 && (old_buttons & mask) == 0 {
                self.event_queue.push_back(InputEvent::MouseButtonDown {
                    button,
                    x: self.mouse_x,
                    y: self.mouse_y,
                });
            } else if (buttons & mask) == 0 && (old_buttons & mask) != 0 {
                self.event_queue.push_back(InputEvent::MouseButtonUp {
                    button,
                    x: self.mouse_x,
                    y: self.mouse_y,
                });
            }
        }
    }
    
    /// 다음 이벤트 가져오기
    pub fn poll_event(&mut self) -> Option<InputEvent> {
        self.event_queue.pop_front()
    }
}
```

---

## 5. 병렬 작업

Phase 3 진행 중 병렬로 수행 가능한 작업:

| 작업 | 의존성 | 비고 |
|------|--------|------|
| 폰트 렌더링 | Canvas 2D | 트루타입 파싱 |
| 오디오 (선택) | IPC 완료 | VirtIO-Sound |
| 추가 GPU 드라이버 | GPU HAL 완료 | NVK, RADV 개선 |

---

## 6. 검증 체크리스트

Phase 3 완료 전 확인 사항:

- [ ] GPU 드라이버 프로세스 분리 동작
- [ ] GPU 드라이버 크래시 후 자동 재시작
- [ ] wgpu 어댑터 열거
- [ ] wgpu 삼각형 렌더링
- [ ] Vello 벡터 그래픽 렌더링
- [ ] Canvas 2D fillRect 동작
- [ ] 윈도우 생성 및 표시
- [ ] 윈도우 이동/크기 조절
- [ ] 키보드 입력 이벤트 전달
- [ ] 마우스 입력 이벤트 전달
- [ ] 폴백 모드 동작 (GPU 컴퓨트 미지원 시)
