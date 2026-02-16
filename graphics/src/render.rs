//! Rendering pipeline abstraction.
//!
//! This module provides render pipeline creation and management.

use alloc::string::String;
use alloc::vec::Vec;

use crate::GraphicsError;

/// Render pipeline handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderPipelineHandle(pub u64);

/// Render pipeline descriptor.
#[derive(Debug, Clone)]
pub struct RenderPipelineDescriptor {
    /// Pipeline label.
    pub label: Option<String>,
    /// Vertex shader.
    pub vertex_shader: ShaderModule,
    /// Fragment shader.
    pub fragment_shader: Option<ShaderModule>,
    /// Vertex buffer layouts.
    pub vertex_buffers: Vec<VertexBufferLayout>,
    /// Primitive topology.
    pub primitive_topology: PrimitiveTopology,
    /// Depth/stencil state.
    pub depth_stencil: Option<DepthStencilState>,
    /// Multisample state.
    pub multisample: MultisampleState,
    /// Color target states.
    pub color_targets: Vec<ColorTargetState>,
}

/// Shader module.
#[derive(Debug, Clone)]
pub struct ShaderModule {
    /// SPIR-V bytecode.
    pub spirv: Vec<u32>,
    /// Entry point name.
    pub entry_point: String,
}

/// Vertex buffer layout.
#[derive(Debug, Clone)]
pub struct VertexBufferLayout {
    /// Stride between elements.
    pub stride: u64,
    /// Step mode.
    pub step_mode: VertexStepMode,
    /// Vertex attributes.
    pub attributes: Vec<VertexAttribute>,
}

/// Vertex step mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexStepMode {
    /// Step per vertex.
    Vertex,
    /// Step per instance.
    Instance,
}

/// Vertex attribute.
#[derive(Debug, Clone)]
pub struct VertexAttribute {
    /// Attribute location.
    pub location: u32,
    /// Byte offset.
    pub offset: u64,
    /// Attribute format.
    pub format: VertexFormat,
}

/// Vertex formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexFormat {
    Float32,
    Float32x2,
    Float32x3,
    Float32x4,
    Uint32,
    Uint32x2,
    Uint32x3,
    Uint32x4,
    Sint32,
    Sint32x2,
    Sint32x3,
    Sint32x4,
    Uint8x2,
    Uint8x4,
    Sint8x2,
    Sint8x4,
    Unorm8x2,
    Unorm8x4,
    Snorm8x2,
    Snorm8x4,
    Uint16x2,
    Uint16x4,
    Sint16x2,
    Sint16x4,
    Unorm16x2,
    Unorm16x4,
    Snorm16x2,
    Snorm16x4,
    Float16x2,
    Float16x4,
}

impl VertexFormat {
    /// Get the size in bytes.
    pub fn size(&self) -> u64 {
        match self {
            VertexFormat::Float32 | VertexFormat::Uint32 | VertexFormat::Sint32 => 4,
            VertexFormat::Float32x2 | VertexFormat::Uint32x2 | VertexFormat::Sint32x2 => 8,
            VertexFormat::Float32x3 | VertexFormat::Uint32x3 | VertexFormat::Sint32x3 => 12,
            VertexFormat::Float32x4 | VertexFormat::Uint32x4 | VertexFormat::Sint32x4 => 16,
            VertexFormat::Uint8x2
            | VertexFormat::Sint8x2
            | VertexFormat::Unorm8x2
            | VertexFormat::Snorm8x2 => 2,
            VertexFormat::Uint8x4
            | VertexFormat::Sint8x4
            | VertexFormat::Unorm8x4
            | VertexFormat::Snorm8x4 => 4,
            VertexFormat::Uint16x2
            | VertexFormat::Sint16x2
            | VertexFormat::Unorm16x2
            | VertexFormat::Snorm16x2
            | VertexFormat::Float16x2 => 4,
            VertexFormat::Uint16x4
            | VertexFormat::Sint16x4
            | VertexFormat::Unorm16x4
            | VertexFormat::Snorm16x4
            | VertexFormat::Float16x4 => 8,
        }
    }
}

/// Primitive topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}

/// Depth/stencil state.
#[derive(Debug, Clone)]
pub struct DepthStencilState {
    /// Depth format.
    pub format: DepthFormat,
    /// Enable depth write.
    pub depth_write_enabled: bool,
    /// Depth compare function.
    pub depth_compare: CompareFunction,
    /// Stencil state.
    pub stencil: Option<StencilState>,
    /// Depth bias.
    pub bias: DepthBias,
}

/// Depth formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthFormat {
    Depth16Unorm,
    Depth24Plus,
    Depth24PlusStencil8,
    Depth32Float,
    Depth32FloatStencil8,
}

/// Compare functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// Stencil state.
#[derive(Debug, Clone)]
pub struct StencilState {
    /// Front face operation.
    pub front: StencilFaceState,
    /// Back face operation.
    pub back: StencilFaceState,
    /// Read mask.
    pub read_mask: u32,
    /// Write mask.
    pub write_mask: u32,
}

/// Stencil face state.
#[derive(Debug, Clone)]
pub struct StencilFaceState {
    /// Compare function.
    pub compare: CompareFunction,
    /// Fail operation.
    pub fail_op: StencilOperation,
    /// Depth fail operation.
    pub depth_fail_op: StencilOperation,
    /// Pass operation.
    pub pass_op: StencilOperation,
}

/// Stencil operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StencilOperation {
    Keep,
    Zero,
    Replace,
    Invert,
    IncrementClamp,
    DecrementClamp,
    IncrementWrap,
    DecrementWrap,
}

/// Depth bias.
#[derive(Debug, Clone, Copy)]
pub struct DepthBias {
    pub constant: i32,
    pub slope_scale: f32,
    pub clamp: f32,
}

impl Default for DepthBias {
    fn default() -> Self {
        DepthBias {
            constant: 0,
            slope_scale: 0.0,
            clamp: 0.0,
        }
    }
}

/// Multisample state.
#[derive(Debug, Clone)]
pub struct MultisampleState {
    /// Sample count (1, 2, 4, 8, 16).
    pub count: u32,
    /// Sample mask.
    pub mask: u64,
    /// Alpha to coverage.
    pub alpha_to_coverage_enabled: bool,
}

impl Default for MultisampleState {
    fn default() -> Self {
        MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }
}

/// Color target state.
#[derive(Debug, Clone)]
pub struct ColorTargetState {
    /// Pixel format.
    pub format: crate::PixelFormat,
    /// Blend state.
    pub blend: Option<BlendState>,
    /// Write mask.
    pub write_mask: ColorWriteMask,
}

/// Blend state.
#[derive(Debug, Clone)]
pub struct BlendState {
    /// Color blend.
    pub color: BlendComponent,
    /// Alpha blend.
    pub alpha: BlendComponent,
}

/// Blend component.
#[derive(Debug, Clone)]
pub struct BlendComponent {
    /// Source factor.
    pub src_factor: BlendFactor,
    /// Destination factor.
    pub dst_factor: BlendFactor,
    /// Blend operation.
    pub operation: BlendOperation,
}

/// Blend factors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
    Constant,
    OneMinusConstant,
}

/// Blend operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

bitflags::bitflags! {
    /// Color write mask.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ColorWriteMask: u32 {
        const RED = 0b0001;
        const GREEN = 0b0010;
        const BLUE = 0b0100;
        const ALPHA = 0b1000;
        const ALL = 0b1111;
    }
}

/// Create a render pipeline.
pub fn create_render_pipeline(
    _descriptor: &RenderPipelineDescriptor,
) -> Result<RenderPipelineHandle, GraphicsError> {
    static mut NEXT_HANDLE: u64 = 1;
    let handle = unsafe {
        let h = NEXT_HANDLE;
        NEXT_HANDLE += 1;
        RenderPipelineHandle(h)
    };

    // Actual pipeline creation would go here
    Ok(handle)
}

/// Destroy a render pipeline.
pub fn destroy_render_pipeline(_handle: RenderPipelineHandle) -> Result<(), GraphicsError> {
    // Actual cleanup would go here
    Ok(())
}
