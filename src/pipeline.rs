use std::{
    borrow::Cow,
    num::{NonZeroU32, NonZeroU64},
};

use bytemuck::{Pod, Zeroable};
use wgpu::{util::DeviceExt, *};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct UniformBufferData {
    pub screen_size: [f32; 2],
}
pub struct SizedBuffer {
    pub buffer: Buffer,
    pub size: usize,
}

pub struct Pipeline {
    pub pipeline: RenderPipeline,

    pub uniform_bind_group_layout: BindGroupLayout,
    pub texture_bind_group_layout: BindGroupLayout,
    pub uniform_bind_group: BindGroup,
    pub uniform_buffer: SizedBuffer,
}
impl Pipeline {
    pub fn new(device: &Device, output_format: TextureFormat, msaa_samples: u32) -> Self {
        create_pipeline(device, output_format, msaa_samples)
    }
}

#[inline(always)]
fn create_pipeline(device: &Device, output_format: TextureFormat, msaa_samples: u32) -> Pipeline {
    let shader = wgpu::ShaderModuleDescriptor {
        label: Some("shader/egui.wgsl"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader/egui.wgsl"))),
    };
    let module = device.create_shader_module(&shader);

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("egui_uniform_buffer"),
        contents: bytemuck::cast_slice(&[UniformBufferData {
            screen_size: [0.0, 0.0],
        }]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let uniform_buffer = SizedBuffer {
        buffer: uniform_buffer,
        size: std::mem::size_of::<UniformBufferData>(),
    };

    let uniform_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("egui_uniform_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: wgpu::BufferBindingType::Uniform,
                },
                count: None,
            }],
        });

    let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("egui_uniform_bind_group"),
        layout: &uniform_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &uniform_buffer.buffer,
                offset: 0,
                size: None,
            }),
        }],
    });

    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("egui_texture_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(4),
                    },
                    count: None,
                },
            ],
        });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("egui_pipeline_layout"),
        bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("egui_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            entry_point: if output_format.describe().srgb {
                "vs_main"
            } else {
                "vs_conv_main"
            },
            module: &module,
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 5 * 4,
                step_mode: wgpu::VertexStepMode::Vertex,
                // 0: vec2 position
                // 1: vec2 texture coordinates
                // 2: uint color
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
            }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            unclipped_depth: false,
            conservative: false,
            cull_mode: None,
            front_face: wgpu::FrontFace::default(),
            polygon_mode: wgpu::PolygonMode::default(),
            strip_index_format: None,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            alpha_to_coverage_enabled: false,
            count: msaa_samples,
            mask: !0,
        },

        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: "fs_main",
            targets: &[wgpu::ColorTargetState {
                format: output_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            }],
        }),
        multiview: None,
    });
    Pipeline {
        pipeline: render_pipeline,
        uniform_bind_group_layout,
        texture_bind_group_layout,
        uniform_bind_group,
        uniform_buffer,
    }
}
