use ahash::AHashMap;
use egui::ClippedMesh;
use std::{
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    rc::Rc,
};
use wgpu::{util::DeviceExt, Buffer, BufferBinding, BufferUsages, Device, RenderPass, TextureView};

use crate::{
    pipeline::{Pipeline, SizedBuffer, UniformBufferData},
    RenderTarget,
};

pub struct Painter {
    sampler: wgpu::Sampler,
    textures: AHashMap<egui::TextureId, wgpu::BindGroup>,
    vertex_buffers: Vec<SizedBuffer>,
    index_buffers: Vec<SizedBuffer>,
    #[cfg(feature = "epi")]
    /// [`egui::TextureId::User`] index
    next_native_tex_id: u64,
}

impl Painter {
    pub fn new(device: &Device) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            ..Default::default()
        });
        Self {
            sampler,
            textures: Default::default(),
            #[cfg(feature = "epi")]
            next_native_tex_id: 0,
            vertex_buffers: Default::default(),
            index_buffers: Default::default(),
        }
    }

    pub fn paint_and_update_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline: &Pipeline,
        target: RenderTarget,
        pixels_per_point: f32,
        clipped_meshes: Vec<egui::ClippedMesh>,
        textures_delta: &egui::TexturesDelta,
    ) {
        for (id, image_delta) in &textures_delta.set {
            self.set_texture(device, queue, pipeline, *id, image_delta);
        }

        self.paint_meshes(
            device,
            queue,
            target,
            pipeline,
            pixels_per_point,
            clipped_meshes,
        );

        for &id in &textures_delta.free {
            self.free_texture(id);
        }
    }

    pub fn set_texture(
        &mut self,
        device: &Device,
        queue: &wgpu::Queue,
        pipeline: &Pipeline,
        tex_id: egui::TextureId,
        delta: &egui::epaint::ImageDelta,
    ) {
        let (data, fmt, size, comps) = match &delta.image {
            egui::ImageData::Color(image) => {
                assert_eq!(
                    image.width() * image.height(),
                    image.pixels.len(),
                    "Mismatch between texture size and texel count"
                );
                (
                    bytemuck::cast_slice(image.pixels.as_slice()),
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    (image.width(), image.height()),
                    4,
                )
            }
            egui::ImageData::Alpha(image) => (
                image.pixels.as_slice(),
                wgpu::TextureFormat::R8Unorm,
                (image.width(), image.height()),
                1u32,
            ),
        };

        let tex = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: size.0 as u32,
                    height: size.1 as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: fmt,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            },
            data,
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            ..Default::default()
        });
        let comps_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&comps),
            usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &comps_buffer,
                        offset: 0,
                        size: NonZeroU64::new(4),
                    }),
                },
            ],
        });
        self.textures.insert(tex_id, bind_group);
    }

    pub fn free_texture(&mut self, id: egui::TextureId) {
        self.textures.remove(&id);
    }

    fn paint_meshes(
        &mut self,
        device: &Device,
        queue: &wgpu::Queue,
        target: RenderTarget,
        pipeline: &Pipeline,
        pixels_per_point: f32,

        clipped_meshes: Vec<egui::ClippedMesh>,
    ) {
        let load = if let Some(color) = target.clear_color.as_ref() {
            wgpu::LoadOp::Clear(*color)
        } else {
            wgpu::LoadOp::Load
        };
        let physical_width = target.width;
        let physical_height = target.height;
        let buffer = &pipeline.uniform_buffer.buffer;
        queue.write_buffer(
            buffer,
            0,
            bytemuck::bytes_of(&UniformBufferData {
                screen_size: [physical_width as f32, physical_height as f32],
            }),
        );

        for (i, ClippedMesh(_, mesh)) in clipped_meshes.iter().enumerate() {
            update_buffer_at(
                device,
                queue,
                i,
                &mut self.vertex_buffers,
                bytemuck::cast_slice(mesh.vertices.as_slice()),
                wgpu::BufferUsages::VERTEX,
            );
            update_buffer_at(
                device,
                queue,
                i,
                &mut self.index_buffers,
                bytemuck::cast_slice(mesh.indices.as_slice()),
                wgpu::BufferUsages::INDEX,
            );
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui-encoder"),
        });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui-rpass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: target.view,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: true },
                }],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&pipeline.pipeline);
            rpass.set_bind_group(0, &pipeline.uniform_bind_group, &[]);

            for (i, ClippedMesh(clip_rect, mesh)) in clipped_meshes.into_iter().enumerate() {
                // Transform clip rect to physical pixels.
                let clip_min_x = pixels_per_point * clip_rect.min.x;
                let clip_min_y = pixels_per_point * clip_rect.min.y;
                let clip_max_x = pixels_per_point * clip_rect.max.x;
                let clip_max_y = pixels_per_point * clip_rect.max.y;

                // Make sure clip rect can fit within an `u32`.
                let clip_min_x = clip_min_x.clamp(0.0, physical_width as f32);
                let clip_min_y = clip_min_y.clamp(0.0, physical_height as f32);
                let clip_max_x = clip_max_x.clamp(clip_min_x, physical_width as f32);
                let clip_max_y = clip_max_y.clamp(clip_min_y, physical_height as f32);

                let clip_min_x = clip_min_x.round() as u32;
                let clip_min_y = clip_min_y.round() as u32;
                let clip_max_x = clip_max_x.round() as u32;
                let clip_max_y = clip_max_y.round() as u32;

                let width = (clip_max_x - clip_min_x).max(1);
                let height = (clip_max_y - clip_min_y).max(1);

                {
                    // Clip scissor rectangle to target size.
                    let x = clip_min_x.min(physical_width);
                    let y = clip_min_y.min(physical_height);
                    let width = width.min(physical_width - x);
                    let height = height.min(physical_height - y);

                    // Skip rendering with zero-sized clip areas.
                    if width == 0 || height == 0 {
                        continue;
                    }
                    rpass.set_scissor_rect(x, y, width, height);
                }
                if let Some(tex_bind) = self.textures.get(&mesh.texture_id) {
                    rpass.set_bind_group(1, &tex_bind, &[]);
                } else {
                    continue;
                }

                let buffer = &self.vertex_buffers[i].buffer;
                rpass.set_vertex_buffer(0, buffer.slice(..));

                let buffer = &self.index_buffers[i].buffer;
                rpass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);

                rpass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
            }
        } //end rpass
        let cmd = encoder.finish();
        queue.submit(Some(cmd));
    }
}

fn update_buffer(
    device: &Device,
    queue: &wgpu::Queue,
    buffer: &mut SizedBuffer,
    data: &[u8],
    usage: wgpu::BufferUsages,
) {
    if buffer.size < data.len() {
        *buffer = create_buffer(device, data, usage);
    } else {
        queue.write_buffer(&buffer.buffer, 0, data);
    }
}

fn create_buffer(device: &Device, data: &[u8], usage: BufferUsages) -> SizedBuffer {
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: data,
        usage: usage | wgpu::BufferUsages::COPY_DST,
    });

    SizedBuffer {
        buffer,
        size: data.len(),
    }
}
fn update_buffer_at(
    device: &Device,
    queue: &wgpu::Queue,
    i: usize,
    buffers: &mut Vec<SizedBuffer>,
    data: &[u8],
    usage: wgpu::BufferUsages,
) {
    if buffers.len() > i {
        let buffer = &mut buffers[i];
        update_buffer(device, queue, buffer, data, usage);
    } else {
        buffers.push(create_buffer(device, data, usage));
    }
}
