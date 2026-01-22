# Graphics Subsystem Design Document

**Document Version:** 1.0.0  
**Last Updated:** 2026-01-21  
**Status:** Initial Draft

---

## Table of Contents

1. [Overview](#1-overview)
2. [Design Principles](#2-design-principles)
3. [Architecture Layers](#3-architecture-layers)
4. [Kernel Graphics Components](#4-kernel-graphics-components)
5. [User-Space Graphics Stack](#5-user-space-graphics-stack)
6. [Compositor Design](#6-compositor-design)
7. [Rendering Pipeline](#7-rendering-pipeline)
8. [Window Management Protocol](#8-window-management-protocol)
9. [WebGPU Integration](#9-webgpu-integration)
10. [OpenGL Compatibility Layer](#10-opengl-compatibility-layer)
11. [Performance Optimizations](#11-performance-optimizations)
12. [Driver Support Matrix](#12-driver-support-matrix)

---

## 1. Overview

### 1.1 Purpose

This document specifies the design of the KPIO graphics subsystem, which implements the "Vulkan Mandate" - a Vulkan-exclusive graphics architecture that provides native GPU performance while maintaining the WebAssembly security model.

### 1.2 Scope

This document covers:
- Kernel-level graphics primitives (DRM/KMS port)
- User-space Vulkan driver integration (Mesa)
- Compositor architecture
- Vector rendering system
- Window management protocol
- WebGPU binding for WASM applications

This document does NOT cover:
- Specific GPU hardware details
- Video codec support (future extension)
- 3D game engine integration

### 1.3 Source Location

```
graphics/
    Cargo.toml
    src/
        lib.rs
        drm/
            mod.rs
            mode.rs         # Mode setting
            buffer.rs       # Buffer management
            gem.rs          # Graphics Execution Manager
            ioctl.rs        # DRM ioctl interface
        compositor/
            mod.rs
            window.rs       # Window abstraction
            surface.rs      # Surface management
            layout.rs       # Window layout algorithms
            input.rs        # Input routing
            animation.rs    # Animation system
        renderer/
            mod.rs
            vello.rs        # Vello integration
            text.rs         # Text rendering
            scene.rs        # Scene graph
```

---

## 2. Design Principles

### 2.1 The Vulkan Mandate

KPIO enforces a **Vulkan-only** graphics policy:

| Principle | Implementation |
|-----------|----------------|
| Single API | All GPU access goes through Vulkan |
| No Legacy | No direct OpenGL/DirectX kernel support |
| Translation | Legacy APIs translated via Zink |
| Consistency | Uniform driver model across vendors |

**Rationale:**
1. **Reduced Complexity:** One API to support, test, and optimize
2. **Modern Design:** Vulkan's explicit resource management matches OS design
3. **Cross-Vendor:** Mesa provides Vulkan for AMD, Intel, NVIDIA
4. **WebGPU Alignment:** WebGPU maps cleanly to Vulkan

### 2.2 User-Space Rendering

All rendering occurs in user space:

```
+-----------------------------------------------------------------+
|                      USER SPACE                                  |
+-----------------------------------------------------------------+
|  Applications  |  Mesa Vulkan  |  Compositor  |  Renderer       |
+-----------------------------------------------------------------+
                              |
                              | DRM/KMS ioctl
                              v
+-----------------------------------------------------------------+
|                      KERNEL                                      |
+-----------------------------------------------------------------+
|                  DRM (Buffer + Mode Setting only)                |
+-----------------------------------------------------------------+
```

### 2.3 Zero-Copy Display

The display pipeline minimizes memory copies:

```
Application Buffer --> Compositor Reference --> Display Scanout
       |                     |                      |
       |                     |                      |
    Created               Zero-copy              Direct
    by app               compositing             flip
```

---

## 3. Architecture Layers

### 3.1 Complete Graphics Stack

```
+=========================================================================+
|                        APPLICATION LAYER                                 |
+=========================================================================+
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  WASM App        |  |  WASM App        |  |  System UI (Shell)     |  |
|  |  (WebGPU API)    |  |  (Canvas 2D)     |  |  (Vector Graphics)     |  |
|  +--------+---------+  +--------+---------+  +-----------+------------+  |
|           |                     |                        |               |
+=========================================================================+
|                        ABSTRACTION LAYER                                 |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                         wgpu-core                                   |  |
|  |            (WebGPU implementation over Vulkan)                      |  |
|  +--------------------------------------------------------------------+  |
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |   Vello          |  |   Canvas (tiny-  |  |   UI Toolkit (iced/   |  |
|  |   (GPU Vector)   |  |   skia or vello) |  |   egui or custom)     |  |
|  +--------+---------+  +--------+---------+  +-----------+------------+  |
|           |                     |                        |               |
|           +---------------------+------------------------+               |
|                                 |                                        |
+=========================================================================+
|                        COMPOSITOR LAYER                                  |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                       KPIO Compositor                               |  |
|  |  - Window management                                                |  |
|  |  - Surface composition                                              |  |
|  |  - Input dispatch                                                   |  |
|  |  - VSync synchronization                                            |  |
|  +-------------------------------+------------------------------------+  |
|                                  |                                       |
+=========================================================================+
|                        VULKAN DRIVER LAYER                               |
+=========================================================================+
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  RADV (AMD)      |  |  ANV (Intel)     |  |  NVK (NVIDIA)          |  |
|  +--------+---------+  +--------+---------+  +-----------+------------+  |
|           |                     |                        |               |
|           +---------------------+------------------------+               |
|                                 |                                        |
|  +--------------------------------------------------------------------+  |
|  |                         Mesa Core                                   |  |
|  |  - Vulkan Loader                                                    |  |
|  |  - SPIR-V Compiler                                                  |  |
|  |  - Shader Cache                                                     |  |
|  +-------------------------------+------------------------------------+  |
|                                  |                                       |
+=========================================================================+
|                        KERNEL LAYER                                      |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                         DRM Subsystem                               |  |
|  |  - GEM (Buffer Object Management)                                   |  |
|  |  - KMS (Kernel Mode Setting)                                        |  |
|  |  - Fence Synchronization                                            |  |
|  +-------------------------------+------------------------------------+  |
|                                  |                                       |
+=========================================================================+
|                        HARDWARE LAYER                                    |
+=========================================================================+
|                                                                          |
|                              GPU                                         |
|                                                                          |
+=========================================================================+
```

### 3.2 Component Responsibilities

| Component | Responsibility |
|-----------|----------------|
| DRM | Buffer allocation, mode setting, sync primitives |
| Mesa Vulkan | GPU command generation, shader compilation |
| wgpu-core | WebGPU API implementation |
| Vello | GPU-accelerated vector rendering |
| Compositor | Window layout, surface composition, input |

---

## 4. Kernel Graphics Components

### 4.1 DRM (Direct Rendering Manager) Port

The kernel provides a minimal DRM interface ported from Linux/BSD:

```rust
// graphics/src/drm/mod.rs

pub struct DrmDevice {
    /// Device file descriptor
    fd: RawFd,
    
    /// Mode resources
    mode_resources: ModeResources,
    
    /// Active framebuffers
    framebuffers: Vec<Framebuffer>,
    
    /// GEM buffer objects
    gem_objects: BTreeMap<u32, GemObject>,
    
    /// Event queue for page flip events
    event_queue: EventQueue,
}

pub struct ModeResources {
    pub connectors: Vec<Connector>,
    pub encoders: Vec<Encoder>,
    pub crtcs: Vec<Crtc>,
    pub planes: Vec<Plane>,
}

pub struct Connector {
    pub id: u32,
    pub connector_type: ConnectorType,
    pub status: ConnectorStatus,
    pub modes: Vec<DisplayMode>,
    pub encoder_id: Option<u32>,
    pub properties: PropertyMap,
}

#[repr(C)]
pub struct DisplayMode {
    pub clock: u32,           // Pixel clock in kHz
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub vrefresh: u32,
    pub flags: ModeFlags,
    pub name: [u8; 32],
}
```

### 4.2 Kernel Mode Setting (KMS)

```rust
// graphics/src/drm/mode.rs

impl DrmDevice {
    /// Get current mode configuration
    pub fn get_resources(&self) -> Result<ModeResources, DrmError> {
        let mut res = ModeResources::default();
        
        // Enumerate CRTCs
        let crtc_ids = self.ioctl_get_resource_ids(ResourceType::Crtc)?;
        for id in crtc_ids {
            res.crtcs.push(self.get_crtc(id)?);
        }
        
        // Enumerate connectors
        let connector_ids = self.ioctl_get_resource_ids(ResourceType::Connector)?;
        for id in connector_ids {
            res.connectors.push(self.get_connector(id)?);
        }
        
        // Enumerate planes
        let plane_ids = self.ioctl_get_resource_ids(ResourceType::Plane)?;
        for id in plane_ids {
            res.planes.push(self.get_plane(id)?);
        }
        
        Ok(res)
    }
    
    /// Set display mode
    pub fn set_mode(
        &mut self,
        crtc_id: u32,
        connector_id: u32,
        mode: &DisplayMode,
        fb_id: u32,
    ) -> Result<(), DrmError> {
        let req = SetCrtcRequest {
            crtc_id,
            fb_id,
            x: 0,
            y: 0,
            connector_ids: &[connector_id],
            mode: Some(mode),
        };
        
        self.ioctl(DRM_IOCTL_SET_CRTC, &req)
    }
    
    /// Atomic mode setting (preferred)
    pub fn atomic_commit(
        &mut self,
        request: AtomicRequest,
        flags: AtomicFlags,
    ) -> Result<(), DrmError> {
        // Build property list
        let props = request.build_property_list();
        
        self.ioctl(DRM_IOCTL_ATOMIC, &AtomicIoctl {
            flags: flags.bits(),
            count_props: props.len() as u32,
            props: props.as_ptr(),
        })
    }
}

pub struct AtomicRequest {
    crtc_props: Vec<(u32, PropertyValue)>,
    connector_props: Vec<(u32, PropertyValue)>,
    plane_props: Vec<(u32, PropertyValue)>,
}

impl AtomicRequest {
    pub fn new() -> Self {
        Self {
            crtc_props: Vec::new(),
            connector_props: Vec::new(),
            plane_props: Vec::new(),
        }
    }
    
    pub fn set_plane_fb(&mut self, plane_id: u32, fb_id: u32) -> &mut Self {
        self.plane_props.push((plane_id, PropertyValue::FbId(fb_id)));
        self
    }
    
    pub fn set_plane_src(&mut self, plane_id: u32, rect: Rect) -> &mut Self {
        // Set SRC_X, SRC_Y, SRC_W, SRC_H properties
        self
    }
    
    pub fn set_plane_crtc(&mut self, plane_id: u32, rect: Rect) -> &mut Self {
        // Set CRTC_X, CRTC_Y, CRTC_W, CRTC_H properties
        self
    }
}
```

### 4.3 Buffer Object Management (GEM)

```rust
// graphics/src/drm/gem.rs

pub struct GemObject {
    /// Unique handle within this device
    pub handle: u32,
    
    /// Size in bytes
    pub size: usize,
    
    /// Physical pages backing this object
    pub pages: Vec<PhysAddr>,
    
    /// CPU mapping (if any)
    pub cpu_mapping: Option<VirtAddr>,
    
    /// Usage flags
    pub flags: GemFlags,
    
    /// Reference count
    refcount: AtomicUsize,
}

bitflags! {
    pub struct GemFlags: u32 {
        const SCANOUT = 1 << 0;      // Can be scanned out to display
        const CURSOR = 1 << 1;       // Suitable for cursor
        const RENDER = 1 << 2;       // GPU rendering target
        const LINEAR = 1 << 3;       // Linear (non-tiled) layout
        const CONTIGUOUS = 1 << 4;   // Physically contiguous
    }
}

impl DrmDevice {
    /// Create a GEM buffer object
    pub fn gem_create(&mut self, size: usize, flags: GemFlags) -> Result<u32, DrmError> {
        let page_count = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        
        // Allocate physical pages
        let pages = if flags.contains(GemFlags::CONTIGUOUS) {
            // Allocate contiguous for DMA
            let order = page_count.next_power_of_two().trailing_zeros() as usize;
            let base = PHYSICAL_MEMORY.allocate(order)
                .ok_or(DrmError::OutOfMemory)?;
            (0..page_count).map(|i| base + i * PAGE_SIZE).collect()
        } else {
            // Non-contiguous allocation
            (0..page_count)
                .map(|_| PHYSICAL_MEMORY.allocate(0))
                .collect::<Option<Vec<_>>>()
                .ok_or(DrmError::OutOfMemory)?
        };
        
        let handle = self.next_gem_handle();
        let obj = GemObject {
            handle,
            size,
            pages,
            cpu_mapping: None,
            flags,
            refcount: AtomicUsize::new(1),
        };
        
        self.gem_objects.insert(handle, obj);
        Ok(handle)
    }
    
    /// Map GEM object for CPU access
    pub fn gem_mmap(&mut self, handle: u32, offset: u64) -> Result<VirtAddr, DrmError> {
        let obj = self.gem_objects.get_mut(&handle)
            .ok_or(DrmError::InvalidHandle)?;
        
        if let Some(addr) = obj.cpu_mapping {
            return Ok(addr);
        }
        
        // Map pages into kernel address space
        let virt = KERNEL_VMALLOC.allocate(obj.size)?;
        for (i, &phys) in obj.pages.iter().enumerate() {
            kernel_page_tables().map(
                virt + i * PAGE_SIZE,
                phys,
                PageFlags::KERNEL_DATA | PageFlags::WRITE_COMBINE,
            )?;
        }
        
        obj.cpu_mapping = Some(virt);
        Ok(virt)
    }
    
    /// Create framebuffer from GEM object
    pub fn add_fb2(
        &mut self,
        width: u32,
        height: u32,
        format: PixelFormat,
        handles: [u32; 4],
        pitches: [u32; 4],
        offsets: [u32; 4],
        modifiers: [u64; 4],
    ) -> Result<u32, DrmError> {
        let fb_id = self.next_fb_id();
        
        let fb = Framebuffer {
            id: fb_id,
            width,
            height,
            format,
            handles,
            pitches,
            offsets,
            modifiers,
        };
        
        self.framebuffers.push(fb);
        Ok(fb_id)
    }
}
```

### 4.4 Fence Synchronization

```rust
// graphics/src/drm/fence.rs

/// DRM sync file for cross-process synchronization
pub struct SyncFile {
    fd: RawFd,
    fence: DrmFence,
}

pub struct DrmFence {
    /// Signaled when GPU completes work
    signaled: AtomicBool,
    
    /// Waiters
    waiters: Mutex<Vec<Waker>>,
    
    /// Seqno for ordering
    seqno: u64,
}

impl DrmFence {
    pub fn wait(&self) -> impl Future<Output = ()> + '_ {
        std::future::poll_fn(|cx| {
            if self.signaled.load(Ordering::SeqCst) {
                Poll::Ready(())
            } else {
                self.waiters.lock().push(cx.waker().clone());
                Poll::Pending
            }
        })
    }
    
    pub fn signal(&self) {
        self.signaled.store(true, Ordering::SeqCst);
        for waker in self.waiters.lock().drain(..) {
            waker.wake();
        }
    }
}

impl DrmDevice {
    /// Create an exportable fence
    pub fn create_fence(&mut self) -> Result<SyncFile, DrmError> {
        let seqno = self.next_fence_seqno();
        let fence = DrmFence {
            signaled: AtomicBool::new(false),
            waiters: Mutex::new(Vec::new()),
            seqno,
        };
        
        let fd = self.create_sync_file_fd(fence)?;
        Ok(SyncFile { fd, fence })
    }
}
```

---

## 5. User-Space Graphics Stack

### 5.1 Mesa 3D Integration

Mesa runs as a user-space library providing Vulkan drivers:

```
+------------------------------------------------------------------+
|                         MESA 3D                                   |
+------------------------------------------------------------------+
|                                                                   |
|  +--------------------+  +--------------------+                   |
|  |   Vulkan Loader    |  |    SPIR-V Tools    |                   |
|  +--------------------+  +--------------------+                   |
|                                                                   |
|  +--------------------+  +--------------------+  +-------------+  |
|  |       RADV         |  |        ANV         |  |     NVK     |  |
|  |   (AMD Vulkan)     |  |  (Intel Vulkan)    |  |   (NVIDIA)  |  |
|  +--------------------+  +--------------------+  +-------------+  |
|                                                                   |
|  +-------------------------------------------------------------+  |
|  |                      NIR (Shader IR)                         |  |
|  +-------------------------------------------------------------+  |
|                                                                   |
|  +-------------------------------------------------------------+  |
|  |                    WSI (Window System Integration)           |  |
|  +-------------------------------------------------------------+  |
|                                                                   |
+------------------------------------------------------------------+
                                  |
                                  | DRM ioctls
                                  v
+------------------------------------------------------------------+
|                         KERNEL                                    |
+------------------------------------------------------------------+
```

### 5.2 WSI (Window System Integration) Port

KPIO provides a custom WSI layer for Mesa:

```c
// Mesa WSI port (conceptual C code for Mesa integration)

/* KPIO-specific WSI implementation */
struct wsi_kpio_image {
    struct wsi_image base;
    uint32_t drm_handle;
    int sync_fd;
};

VkResult wsi_kpio_create_swapchain(
    VkDevice device,
    const VkSwapchainCreateInfoKHR *create_info,
    struct wsi_swapchain **swapchain_out
) {
    struct wsi_kpio_swapchain *chain = calloc(1, sizeof(*chain));
    
    // Connect to compositor
    chain->compositor_connection = kpio_compositor_connect();
    
    // Create surface in compositor
    chain->surface_id = kpio_surface_create(
        chain->compositor_connection,
        create_info->imageExtent.width,
        create_info->imageExtent.height
    );
    
    // Allocate swapchain images
    chain->image_count = create_info->minImageCount;
    for (uint32_t i = 0; i < chain->image_count; i++) {
        // Create GEM buffer via DRM
        chain->images[i].drm_handle = drm_gem_create(
            chain->drm_fd,
            create_info->imageExtent.width * 
            create_info->imageExtent.height * 4
        );
        
        // Export as DMA-BUF for compositor
        chain->images[i].dmabuf_fd = drm_prime_handle_to_fd(
            chain->drm_fd,
            chain->images[i].drm_handle
        );
        
        // Register with compositor
        kpio_surface_attach_buffer(
            chain->surface_id,
            chain->images[i].dmabuf_fd
        );
    }
    
    *swapchain_out = &chain->base;
    return VK_SUCCESS;
}

VkResult wsi_kpio_queue_present(
    struct wsi_swapchain *swapchain,
    uint32_t image_index,
    VkFence fence
) {
    struct wsi_kpio_swapchain *chain = (void *)swapchain;
    
    // Export fence as sync_fd
    int sync_fd = vk_fence_export_sync_fd(fence);
    
    // Tell compositor to display this buffer
    kpio_surface_present(
        chain->surface_id,
        image_index,
        sync_fd
    );
    
    return VK_SUCCESS;
}
```

### 5.3 WASM Vulkan Binding

WASM applications access Vulkan via WebGPU abstraction:

```rust
// runtime/src/gpu/webgpu_binding.rs

/// WebGPU device backed by Vulkan
pub struct WgpuDevice {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl WgpuDevice {
    pub fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        
        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            },
        )).ok_or(GpuError::NoAdapter)?;
        
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("KPIO WebGPU Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))?;
        
        Ok(Self { instance, adapter, device, queue })
    }
}
```

---

## 6. Compositor Design

### 6.1 Compositor Architecture

```rust
// graphics/src/compositor/mod.rs

pub struct Compositor {
    /// GPU device for rendering
    gpu: WgpuDevice,
    
    /// All managed surfaces
    surfaces: BTreeMap<SurfaceId, Surface>,
    
    /// Window tree (spatial hierarchy)
    window_tree: WindowTree,
    
    /// Current output configuration
    outputs: Vec<Output>,
    
    /// Input state
    input_state: InputState,
    
    /// Frame scheduler
    frame_scheduler: FrameScheduler,
    
    /// Vector renderer (Vello)
    renderer: VelloRenderer,
}

pub struct Surface {
    /// Unique identifier
    id: SurfaceId,
    
    /// Owning client
    client_id: ClientId,
    
    /// Current buffer
    current_buffer: Option<Buffer>,
    
    /// Pending buffer (waiting for frame)
    pending_buffer: Option<Buffer>,
    
    /// Surface state
    state: SurfaceState,
    
    /// Damage regions
    damage: DamageTracker,
}

#[derive(Debug, Clone)]
pub struct SurfaceState {
    pub position: Point,
    pub size: Size,
    pub scale: f32,
    pub transform: Transform,
    pub alpha: f32,
    pub visible: bool,
    pub input_region: Region,
    pub opaque_region: Region,
}

pub struct Window {
    /// Surface ID
    surface_id: SurfaceId,
    
    /// Window decorations
    decorations: Decorations,
    
    /// Window properties
    title: String,
    app_id: String,
    
    /// Window state
    state: WindowState,
    
    /// Geometry
    geometry: Rect,
    
    /// Parent (for popups)
    parent: Option<WindowId>,
}

#[derive(Debug, Clone, Copy)]
pub enum WindowState {
    Normal,
    Maximized,
    Minimized,
    Fullscreen,
    Tiled { edges: TileEdges },
}
```

### 6.2 Compositor Main Loop

```rust
// graphics/src/compositor/mod.rs

impl Compositor {
    pub async fn run(&mut self) -> ! {
        loop {
            // Wait for next frame
            self.frame_scheduler.wait_for_vblank().await;
            
            // Process client requests
            self.process_client_events();
            
            // Process input events
            self.dispatch_input_events();
            
            // Commit pending surface updates
            self.commit_surfaces();
            
            // Render frame
            self.render_frame();
            
            // Present to display
            self.present();
        }
    }
    
    fn render_frame(&mut self) {
        let mut scene = Scene::new();
        
        // Build scene from window tree
        for window in self.window_tree.visible_windows() {
            let surface = self.surfaces.get(&window.surface_id).unwrap();
            
            // Add window decorations
            if window.decorations.enabled {
                self.render_decorations(&mut scene, window);
            }
            
            // Add window content
            scene.push_layer(
                window.geometry,
                surface.current_buffer.as_ref(),
                surface.state.alpha,
            );
        }
        
        // Render cursor
        self.render_cursor(&mut scene);
        
        // Encode scene to GPU commands
        self.renderer.render(&scene, &self.gpu.device, &self.gpu.queue);
    }
    
    fn present(&mut self) {
        for output in &mut self.outputs {
            // Get rendered texture
            let texture = self.renderer.output_texture(output.id);
            
            // Submit to display
            output.present(texture);
        }
    }
}
```

### 6.3 Window Layout

```rust
// graphics/src/compositor/layout.rs

pub struct WindowTree {
    root: LayoutNode,
    focus_stack: Vec<WindowId>,
}

pub enum LayoutNode {
    Window(WindowId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        children: Box<(LayoutNode, LayoutNode)>,
    },
    Stack(Vec<LayoutNode>),
}

#[derive(Debug, Clone, Copy)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

impl WindowTree {
    pub fn layout(&mut self, area: Rect) {
        self.layout_node(&mut self.root, area);
    }
    
    fn layout_node(&mut self, node: &mut LayoutNode, area: Rect) {
        match node {
            LayoutNode::Window(id) => {
                if let Some(window) = self.windows.get_mut(id) {
                    window.geometry = area;
                }
            }
            LayoutNode::Split { direction, ratio, children } => {
                let (first_area, second_area) = match direction {
                    SplitDirection::Horizontal => {
                        let split = area.x + (area.width as f32 * ratio) as i32;
                        (
                            Rect::new(area.x, area.y, split - area.x, area.height),
                            Rect::new(split, area.y, area.x + area.width - split, area.height),
                        )
                    }
                    SplitDirection::Vertical => {
                        let split = area.y + (area.height as f32 * ratio) as i32;
                        (
                            Rect::new(area.x, area.y, area.width, split - area.y),
                            Rect::new(area.x, split, area.width, area.y + area.height - split),
                        )
                    }
                };
                self.layout_node(&mut children.0, first_area);
                self.layout_node(&mut children.1, second_area);
            }
            LayoutNode::Stack(children) => {
                for child in children {
                    self.layout_node(child, area);
                }
            }
        }
    }
}
```

---

## 7. Rendering Pipeline

### 7.1 Vello Vector Renderer

```rust
// graphics/src/renderer/vello.rs

use vello::{Scene, RenderParams, Renderer, RendererOptions};
use vello::kurbo::{Affine, Rect, RoundedRect};
use vello::peniko::{Brush, Color, Fill};

pub struct VelloRenderer {
    renderer: Renderer,
    scene: Scene,
}

impl VelloRenderer {
    pub fn new(device: &wgpu::Device) -> Result<Self, RenderError> {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                surface_format: Some(wgpu::TextureFormat::Bgra8Unorm),
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: None,
            },
        )?;
        
        Ok(Self {
            renderer,
            scene: Scene::new(),
        })
    }
    
    pub fn render_window_decoration(
        &mut self,
        window: &Window,
        focused: bool,
    ) {
        let geo = window.geometry;
        
        // Title bar background
        let title_height = 30.0;
        let title_rect = Rect::new(
            geo.x as f64,
            geo.y as f64 - title_height,
            (geo.x + geo.width) as f64,
            geo.y as f64,
        );
        
        let bg_color = if focused {
            Color::rgb8(48, 48, 48)
        } else {
            Color::rgb8(32, 32, 32)
        };
        
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(bg_color),
            None,
            &RoundedRect::from_rect(title_rect, 8.0),
        );
        
        // Window title text
        self.render_text(
            &window.title,
            (geo.x + 10) as f64,
            (geo.y as f64 - title_height / 2.0),
            14.0,
            Color::WHITE,
        );
        
        // Close button
        self.render_close_button(
            (geo.x + geo.width - 30) as f64,
            geo.y as f64 - title_height + 5.0,
        );
        
        // Window border
        let border_color = if focused {
            Color::rgb8(100, 100, 255)
        } else {
            Color::rgb8(64, 64, 64)
        };
        
        let border_rect = Rect::new(
            geo.x as f64 - 1.0,
            geo.y as f64 - title_height - 1.0,
            (geo.x + geo.width) as f64 + 1.0,
            (geo.y + geo.height) as f64 + 1.0,
        );
        
        self.scene.stroke(
            &vello::peniko::Stroke::new(2.0),
            Affine::IDENTITY,
            &Brush::Solid(border_color),
            None,
            &RoundedRect::from_rect(border_rect, 8.0),
        );
    }
    
    pub fn composite_surface(
        &mut self,
        surface: &Surface,
        transform: Affine,
    ) {
        if let Some(buffer) = &surface.current_buffer {
            self.scene.draw_image(
                &buffer.as_image(),
                transform,
            );
        }
    }
    
    pub fn render_to_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        self.renderer.render_to_texture(
            device,
            queue,
            &self.scene,
            texture,
            &RenderParams {
                base_color: Color::BLACK,
                width,
                height,
                antialiasing_method: vello::AaConfig::Msaa16,
            },
        )?;
        
        self.scene.reset();
        Ok(())
    }
}
```

### 7.2 Damage Tracking

```rust
// graphics/src/compositor/damage.rs

pub struct DamageTracker {
    /// Current frame damage
    current: Region,
    
    /// Previous frame damage (for double buffering)
    previous: Region,
    
    /// Full damage flag
    full_damage: bool,
}

pub struct Region {
    rects: Vec<Rect>,
}

impl DamageTracker {
    pub fn damage(&mut self, rect: Rect) {
        if !self.full_damage {
            self.current.add(rect);
        }
    }
    
    pub fn damage_full(&mut self) {
        self.full_damage = true;
    }
    
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.current, &mut self.previous);
        self.current.clear();
        self.full_damage = false;
    }
    
    pub fn combined_damage(&self) -> Region {
        let mut combined = self.current.clone();
        combined.union(&self.previous);
        combined
    }
}

impl Region {
    pub fn add(&mut self, rect: Rect) {
        // Simple: just add the rect
        // TODO: Merge overlapping rects
        self.rects.push(rect);
    }
    
    pub fn union(&mut self, other: &Region) {
        self.rects.extend(other.rects.iter().cloned());
    }
    
    pub fn clear(&mut self) {
        self.rects.clear();
    }
    
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }
}
```

---

## 8. Window Management Protocol

### 8.1 Protocol Overview

Communication between applications and compositor uses IPC:

```rust
// graphics/src/compositor/protocol.rs

/// Messages from client to compositor
#[derive(Debug, Serialize, Deserialize)]
pub enum CompositorRequest {
    // Surface management
    CreateSurface { id: SurfaceId },
    DestroySurface { id: SurfaceId },
    
    // Buffer management
    AttachBuffer { surface: SurfaceId, buffer: BufferDesc },
    Commit { surface: SurfaceId },
    
    // Window management
    SetTitle { surface: SurfaceId, title: String },
    SetAppId { surface: SurfaceId, app_id: String },
    SetMaximized { surface: SurfaceId, maximized: bool },
    SetFullscreen { surface: SurfaceId, fullscreen: bool },
    SetMinimized { surface: SurfaceId },
    Move { surface: SurfaceId },
    Resize { surface: SurfaceId, edges: ResizeEdges },
    Close { surface: SurfaceId },
    
    // Popup/dialog
    CreatePopup { parent: SurfaceId, popup: SurfaceId, config: PopupConfig },
    
    // Input
    SetInputRegion { surface: SurfaceId, region: Region },
    SetCursor { surface: SurfaceId, hotspot: Point },
    HideCursor,
}

/// Messages from compositor to client
#[derive(Debug, Serialize, Deserialize)]
pub enum CompositorEvent {
    // Surface events
    Configure { surface: SurfaceId, config: SurfaceConfig },
    Close { surface: SurfaceId },
    
    // Window state
    StateChanged { surface: SurfaceId, state: WindowState },
    
    // Input events
    PointerEnter { surface: SurfaceId, x: f64, y: f64 },
    PointerLeave { surface: SurfaceId },
    PointerMotion { x: f64, y: f64, time: u32 },
    PointerButton { button: u32, state: ButtonState, time: u32 },
    PointerScroll { axis: Axis, value: f64, time: u32 },
    
    KeyboardEnter { surface: SurfaceId, keys: Vec<u32> },
    KeyboardLeave { surface: SurfaceId },
    KeyboardKey { key: u32, state: KeyState, time: u32 },
    KeyboardModifiers { mods: Modifiers },
    
    // Frame callback
    FrameDone { surface: SurfaceId, time: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceConfig {
    pub width: u32,
    pub height: u32,
    pub states: Vec<SurfaceStateFlag>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SurfaceStateFlag {
    Maximized,
    Fullscreen,
    Resizing,
    Activated,
    TiledLeft,
    TiledRight,
    TiledTop,
    TiledBottom,
}
```

### 8.2 Client Connection

```rust
// graphics/src/compositor/client.rs

pub struct CompositorClient {
    /// IPC channel to compositor
    channel: IpcChannel,
    
    /// Pending events
    events: VecDeque<CompositorEvent>,
    
    /// Registered surfaces
    surfaces: BTreeMap<SurfaceId, ClientSurface>,
}

impl CompositorClient {
    pub async fn connect() -> Result<Self, CompositorError> {
        let channel = IpcChannel::connect("compositor")?;
        
        Ok(Self {
            channel,
            events: VecDeque::new(),
            surfaces: BTreeMap::new(),
        })
    }
    
    pub fn create_surface(&mut self) -> Result<SurfaceId, CompositorError> {
        let id = SurfaceId::new();
        self.channel.send(CompositorRequest::CreateSurface { id })?;
        self.surfaces.insert(id, ClientSurface::new());
        Ok(id)
    }
    
    pub fn attach_buffer(
        &mut self,
        surface: SurfaceId,
        buffer: &Buffer,
    ) -> Result<(), CompositorError> {
        self.channel.send(CompositorRequest::AttachBuffer {
            surface,
            buffer: buffer.describe(),
        })
    }
    
    pub fn commit(&mut self, surface: SurfaceId) -> Result<(), CompositorError> {
        self.channel.send(CompositorRequest::Commit { surface })
    }
    
    pub async fn next_event(&mut self) -> Option<CompositorEvent> {
        if let Some(event) = self.events.pop_front() {
            return Some(event);
        }
        
        // Wait for more events
        let msg = self.channel.recv().await.ok()?;
        self.events.extend(msg.events);
        self.events.pop_front()
    }
}
```

---

## 9. WebGPU Integration

### 9.1 WASM WebGPU API

```rust
// runtime/src/gpu/webgpu_wasi.rs

/// WebGPU WASI extension for WASM applications
pub mod wasi_webgpu {
    use wasmtime::*;
    
    pub fn add_to_linker(linker: &mut Linker<WasiCtx>) -> Result<()> {
        // Instance
        linker.func_wrap("webgpu", "request_adapter", request_adapter)?;
        linker.func_wrap("webgpu", "request_device", request_device)?;
        
        // Device
        linker.func_wrap("webgpu", "create_buffer", create_buffer)?;
        linker.func_wrap("webgpu", "create_texture", create_texture)?;
        linker.func_wrap("webgpu", "create_sampler", create_sampler)?;
        linker.func_wrap("webgpu", "create_bind_group_layout", create_bind_group_layout)?;
        linker.func_wrap("webgpu", "create_bind_group", create_bind_group)?;
        linker.func_wrap("webgpu", "create_pipeline_layout", create_pipeline_layout)?;
        linker.func_wrap("webgpu", "create_render_pipeline", create_render_pipeline)?;
        linker.func_wrap("webgpu", "create_compute_pipeline", create_compute_pipeline)?;
        linker.func_wrap("webgpu", "create_command_encoder", create_command_encoder)?;
        
        // Command encoder
        linker.func_wrap("webgpu", "begin_render_pass", begin_render_pass)?;
        linker.func_wrap("webgpu", "begin_compute_pass", begin_compute_pass)?;
        linker.func_wrap("webgpu", "copy_buffer_to_buffer", copy_buffer_to_buffer)?;
        linker.func_wrap("webgpu", "copy_buffer_to_texture", copy_buffer_to_texture)?;
        linker.func_wrap("webgpu", "copy_texture_to_buffer", copy_texture_to_buffer)?;
        linker.func_wrap("webgpu", "finish_command_encoder", finish_command_encoder)?;
        
        // Queue
        linker.func_wrap("webgpu", "queue_submit", queue_submit)?;
        linker.func_wrap("webgpu", "queue_write_buffer", queue_write_buffer)?;
        linker.func_wrap("webgpu", "queue_write_texture", queue_write_texture)?;
        
        // Surface (compositor integration)
        linker.func_wrap("webgpu", "create_surface", create_surface)?;
        linker.func_wrap("webgpu", "configure_surface", configure_surface)?;
        linker.func_wrap("webgpu", "get_current_texture", get_current_texture)?;
        linker.func_wrap("webgpu", "present", present)?;
        
        Ok(())
    }
}

fn create_buffer(
    mut caller: Caller<'_, WasiCtx>,
    desc_ptr: u32,
) -> Result<u32> {
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("missing memory"))?;
    
    // Read descriptor from WASM memory
    let desc: BufferDescriptor = memory.read_struct(desc_ptr)?;
    
    // Create buffer via wgpu
    let gpu = caller.data().gpu.as_ref().ok_or_else(|| anyhow!("no gpu"))?;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: desc.size,
        usage: wgpu::BufferUsages::from_bits(desc.usage).unwrap(),
        mapped_at_creation: desc.mapped_at_creation,
    });
    
    // Store and return handle
    let handle = caller.data_mut().gpu_resources.insert_buffer(buffer);
    Ok(handle)
}
```

### 9.2 Surface Creation for WASM

```rust
// runtime/src/gpu/surface.rs

fn create_surface(
    mut caller: Caller<'_, WasiCtx>,
    width: u32,
    height: u32,
) -> Result<u32> {
    let compositor = &caller.data().compositor_client;
    
    // Request surface from compositor
    let surface_id = compositor.create_surface()?;
    
    // Create wgpu surface backed by compositor buffer
    let gpu = caller.data().gpu.as_ref().ok_or_else(|| anyhow!("no gpu"))?;
    
    let surface = WasmSurface {
        id: surface_id,
        compositor: compositor.clone(),
        width,
        height,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        textures: Vec::new(),
    };
    
    let handle = caller.data_mut().gpu_resources.insert_surface(surface);
    Ok(handle)
}

fn get_current_texture(
    mut caller: Caller<'_, WasiCtx>,
    surface_handle: u32,
) -> Result<u32> {
    let surface = caller.data().gpu_resources.get_surface(surface_handle)?;
    
    // Get or create texture for current frame
    let texture = surface.acquire_texture()?;
    
    let handle = caller.data_mut().gpu_resources.insert_texture_view(texture);
    Ok(handle)
}

fn present(
    mut caller: Caller<'_, WasiCtx>,
    surface_handle: u32,
) -> Result<()> {
    let surface = caller.data_mut().gpu_resources.get_surface_mut(surface_handle)?;
    
    // Submit current texture to compositor
    surface.present()?;
    
    Ok(())
}
```

---

## 10. OpenGL Compatibility Layer

### 10.1 Zink Translation

OpenGL applications are supported through Zink (OpenGL-to-Vulkan translation):

```
+------------------------------------------------------------------+
|                    LEGACY APPLICATION                             |
|                     (OpenGL calls)                                |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                        ZINK DRIVER                                |
|           (Mesa's OpenGL-to-Vulkan translator)                    |
+------------------------------------------------------------------+
|   - Translates GL state to Vulkan state                          |
|   - Converts GLSL to SPIR-V                                      |
|   - Maps GL objects to Vulkan objects                            |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                      VULKAN DRIVER                                |
|                   (RADV / ANV / NVK)                              |
+------------------------------------------------------------------+
```

### 10.2 Zink Integration

```rust
// graphics/src/gl_compat/mod.rs

/// Configuration for Zink-based OpenGL support
pub struct ZinkConfig {
    /// Enable Zink (default: true)
    pub enabled: bool,
    
    /// OpenGL version to expose
    pub gl_version: GlVersion,
    
    /// GLSL version to support
    pub glsl_version: GlslVersion,
    
    /// Enable compatibility profile (for legacy apps)
    pub compatibility_profile: bool,
}

impl Default for ZinkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gl_version: GlVersion::new(4, 6),
            glsl_version: GlslVersion::new(4, 60),
            compatibility_profile: false,
        }
    }
}

pub struct GlVersion {
    pub major: u8,
    pub minor: u8,
}

pub struct GlslVersion {
    pub major: u8,
    pub minor: u8,
}
```

---

## 11. Performance Optimizations

### 11.1 Zero-Copy Display

```rust
// graphics/src/compositor/scanout.rs

pub struct DirectScanout {
    /// Planes capable of direct scanout
    scanout_planes: Vec<PlaneId>,
    
    /// Surfaces currently using direct scanout
    direct_surfaces: BTreeMap<SurfaceId, PlaneId>,
}

impl DirectScanout {
    /// Attempt to assign surface to direct scanout plane
    pub fn try_direct_scanout(
        &mut self,
        surface: &Surface,
        output: &Output,
    ) -> bool {
        // Check if surface covers entire output
        if surface.state.size != output.size {
            return false;
        }
        
        // Check if surface is opaque
        if surface.state.alpha != 1.0 {
            return false;
        }
        
        // Check buffer format compatibility
        if let Some(buffer) = &surface.current_buffer {
            if !output.supported_formats.contains(&buffer.format) {
                return false;
            }
            
            // Check modifier compatibility
            if let Some(modifier) = buffer.modifier {
                if !output.supported_modifiers.contains(&modifier) {
                    return false;
                }
            }
        } else {
            return false;
        }
        
        // Find available scanout plane
        if let Some(plane) = self.find_available_plane(output) {
            self.direct_surfaces.insert(surface.id, plane);
            return true;
        }
        
        false
    }
}
```

### 11.2 Frame Scheduling

```rust
// graphics/src/compositor/frame.rs

pub struct FrameScheduler {
    /// Target frame rate per output
    target_rates: BTreeMap<OutputId, u32>,
    
    /// VBlank event sources
    vblank_sources: BTreeMap<OutputId, VBlankSource>,
    
    /// Frame timing statistics
    stats: FrameStats,
}

pub struct FrameStats {
    pub frame_count: u64,
    pub total_render_time: Duration,
    pub total_present_time: Duration,
    pub missed_frames: u64,
}

impl FrameScheduler {
    pub async fn wait_for_vblank(&self) -> VBlankEvent {
        // Wait for any output's vblank
        let futures: Vec<_> = self.vblank_sources.values()
            .map(|s| s.wait())
            .collect();
        
        futures::future::select_all(futures).await.0
    }
    
    pub fn record_frame(&mut self, render_time: Duration, present_time: Duration) {
        self.stats.frame_count += 1;
        self.stats.total_render_time += render_time;
        self.stats.total_present_time += present_time;
        
        // Check if we missed the deadline
        let target_period = Duration::from_secs(1) / 60;
        if render_time + present_time > target_period {
            self.stats.missed_frames += 1;
        }
    }
}
```

### 11.3 Buffer Caching

```rust
// graphics/src/compositor/buffer_cache.rs

pub struct BufferCache {
    /// Cached buffers by format and size
    pools: BTreeMap<BufferKey, BufferPool>,
    
    /// Maximum cache size
    max_size: usize,
    
    /// Current cache size
    current_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BufferKey {
    width: u32,
    height: u32,
    format: PixelFormat,
    usage: BufferUsage,
}

struct BufferPool {
    free: Vec<Buffer>,
    in_use: Vec<Buffer>,
}

impl BufferCache {
    pub fn acquire(&mut self, key: BufferKey) -> Buffer {
        if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(buffer) = pool.free.pop() {
                pool.in_use.push(buffer.clone());
                return buffer;
            }
        }
        
        // Allocate new buffer
        let buffer = Buffer::allocate(key.width, key.height, key.format, key.usage);
        
        let pool = self.pools.entry(key).or_insert_with(BufferPool::new);
        pool.in_use.push(buffer.clone());
        
        self.current_size += buffer.size();
        self.maybe_evict();
        
        buffer
    }
    
    pub fn release(&mut self, buffer: Buffer) {
        let key = buffer.cache_key();
        if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(idx) = pool.in_use.iter().position(|b| b.id == buffer.id) {
                let buffer = pool.in_use.remove(idx);
                pool.free.push(buffer);
            }
        }
    }
}
```

---

## 12. Driver Support Matrix

### 12.1 Supported GPUs

| Vendor | Driver | Architecture | Status |
|--------|--------|--------------|--------|
| AMD | RADV | GCN 1.0+ | Tier 1 |
| AMD | RADV | RDNA 1/2/3 | Tier 1 |
| Intel | ANV | Gen 9+ (Skylake) | Tier 1 |
| Intel | ANV | Xe (Gen 12+) | Tier 1 |
| NVIDIA | NVK | Turing+ | Tier 2 |
| NVIDIA | NVK | Ampere+ | Tier 2 |
| Software | llvmpipe | N/A | Tier 3 |

**Tier Definitions:**
- Tier 1: Full support, actively tested
- Tier 2: Supported, limited testing
- Tier 3: Fallback only, limited features

### 12.2 Required Vulkan Features

| Feature | Requirement | Purpose |
|---------|-------------|---------|
| Vulkan 1.2 | Required | Baseline API version |
| VK_KHR_swapchain | Required | Display output |
| VK_KHR_maintenance1 | Required | Basic functionality |
| VK_EXT_external_memory | Required | Buffer sharing |
| VK_KHR_synchronization2 | Recommended | Modern sync |
| VK_EXT_mesh_shader | Optional | Advanced rendering |
| VK_KHR_ray_tracing | Optional | Ray tracing support |

### 12.3 Display Protocol Support

| Protocol | Status | Notes |
|----------|--------|-------|
| DRM Atomic | Required | Modern mode setting |
| DRM Legacy | Not supported | - |
| VRR/FreeSync | Planned | Variable refresh |
| HDR | Planned | High dynamic range |

---

## Appendix A: Pixel Formats

```rust
// graphics/src/drm/formats.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PixelFormat {
    // 32-bit RGBA
    Argb8888 = fourcc(b"AR24"),
    Xrgb8888 = fourcc(b"XR24"),
    Abgr8888 = fourcc(b"AB24"),
    Xbgr8888 = fourcc(b"XB24"),
    
    // 32-bit RGBA (10-bit per channel)
    Argb2101010 = fourcc(b"AR30"),
    Xrgb2101010 = fourcc(b"XR30"),
    
    // 16-bit RGB
    Rgb565 = fourcc(b"RG16"),
    
    // YUV formats (for video)
    Nv12 = fourcc(b"NV12"),
    Yuyv = fourcc(b"YUYV"),
}

const fn fourcc(code: &[u8; 4]) -> u32 {
    (code[0] as u32) |
    ((code[1] as u32) << 8) |
    ((code[2] as u32) << 16) |
    ((code[3] as u32) << 24)
}
```

---

## Appendix B: Compositor Protocol Schema

```
# Compositor Protocol (IPC Message Types)

## Surface Lifecycle

CREATE_SURFACE:
  -> id: SurfaceId
  <- success | error

DESTROY_SURFACE:
  -> id: SurfaceId
  <- success | error

## Buffer Operations

ATTACH_BUFFER:
  -> surface: SurfaceId
  -> buffer:
       type: "dmabuf" | "shm"
       fd: i32 (for dmabuf)
       offset: u32
       width: u32
       height: u32
       stride: u32
       format: PixelFormat
  <- success | error

COMMIT:
  -> surface: SurfaceId
  <- success | error

## Window Management

SET_TITLE:
  -> surface: SurfaceId
  -> title: String
  <- success | error

CONFIGURE:
  <- surface: SurfaceId
  <- width: u32
  <- height: u32
  <- states: [SurfaceState]

## Input Events

POINTER_MOTION:
  <- surface: SurfaceId
  <- x: f64
  <- y: f64
  <- time: u32

KEYBOARD_KEY:
  <- surface: SurfaceId
  <- key: u32
  <- state: Pressed | Released
  <- time: u32
```
