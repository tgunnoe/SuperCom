//! The GPU side: a renderer that draws a terrain mesh and a water plane for
//! a camera into any texture view of one format.

use crate::terrain::VERTEX_BYTES;
use crate::{Camera, Error, TerrainMesh, WaterPlane, grid_fits, grid_vertex_count, shader};
use std::num::NonZeroU64;
use wgpu::{
    Adapter, Backend, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
    CompareFunction, DepthStencilState, Device, DeviceDescriptor, ErrorFilter, Extent3d, Face,
    Features, FragmentState, FrontFace, IndexFormat, Instance, InstanceDescriptor, Limits, LoadOp,
    MultisampleState, Operations, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology,
    Queue, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, StoreOp, Surface, SurfaceTarget, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

/// The depth target's format.
const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// Floats in the uniform: the 4 by 4 view-projection matrix, then the
/// lowest and highest terrain elevation and two zeros.
const UNIFORM_FLOATS: usize = 20;

const VERTEX_ATTRIBUTES: [VertexAttribute; 2] = [
    VertexAttribute {
        format: VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    VertexAttribute {
        format: VertexFormat::Float32x3,
        offset: 12,
        shader_location: 1,
    },
];

/// Runs `make` with a validation and an out-of-memory error scope open and
/// returns what it made, or the first error either scope captured. The
/// device's default handler panics on an uncaptured error, so every call that
/// creates a resource or records work goes through here.
fn scoped<T>(device: &Device, during: &'static str, make: impl FnOnce() -> T) -> Result<T, Error> {
    let memory = device.push_error_scope(ErrorFilter::OutOfMemory);
    let validation = device.push_error_scope(ErrorFilter::Validation);
    let output = make();
    let (invalid, exhausted) =
        pollster::block_on(async { (validation.pop().await, memory.pop().await) });
    match invalid.or(exhausted) {
        Some(error) => Err(Error::Gpu {
            during,
            reason: error.to_string(),
        }),
        None => Ok(output),
    }
}

/// One uploaded mesh.
#[derive(Debug)]
struct GpuMesh {
    vertices: Buffer,
    indices: Buffer,
    index_count: u32,
}

/// What [`Renderer::upload`] left on the device.
#[derive(Debug)]
struct Scene {
    terrain: GpuMesh,
    water: Option<GpuMesh>,
    range: [f32; 2],
}

/// The depth target for one size.
#[derive(Debug)]
struct DepthTarget {
    size: (u32, u32),
    view: TextureView,
}

/// The renderer: one device and queue, built for one target format.
#[derive(Debug)]
pub struct Renderer {
    device: Device,
    queue: Queue,
    adapter_name: String,
    backend: Backend,
    format: TextureFormat,
    terrain_pipeline: RenderPipeline,
    water_pipeline: RenderPipeline,
    uniform: Buffer,
    bind_group: BindGroup,
    depth: Option<DepthTarget>,
    scene: Option<Scene>,
}

/// A renderer built for a surface, with the surface and the format chosen
/// from its capabilities.
#[derive(Debug)]
pub struct SurfaceRenderer {
    /// The renderer, whose device the caller configures `surface` with.
    pub renderer: Renderer,
    /// The surface created from the caller's target.
    pub surface: Surface<'static>,
    /// The surface format the renderer draws in: an sRGB format when the
    /// surface offers one.
    pub format: TextureFormat,
}

/// The format the renderer draws in for a surface that offers `offered`, in
/// the surface's order: the first sRGB format when there is one, otherwise
/// the first format.
///
/// # Errors
/// [`Error::Surface`] when the surface offers no format.
pub fn surface_format(offered: &[TextureFormat]) -> Result<TextureFormat, Error> {
    let _ = offered;
    Ok(TextureFormat::Rgba8Unorm)
}

async fn open(
    instance: &Instance,
    compatible: Option<&Surface<'_>>,
) -> Result<(Adapter, Device, Queue), Error> {
    let options = RequestAdapterOptions {
        compatible_surface: compatible,
        ..RequestAdapterOptions::default()
    };
    let adapter = instance
        .request_adapter(&options)
        .await
        .map_err(|error| Error::NoAdapter {
            reason: error.to_string(),
        })?;
    let descriptor = DeviceDescriptor {
        label: Some("supercom-render"),
        required_features: Features::empty(),
        required_limits: Limits::default().or_worse_values_from(&adapter.limits()),
        ..DeviceDescriptor::default()
    };
    let (device, queue) =
        adapter
            .request_device(&descriptor)
            .await
            .map_err(|error| Error::DeviceRequest {
                reason: error.to_string(),
            })?;
    Ok((adapter, device, queue))
}

impl Renderer {
    fn from_parts(
        adapter: &Adapter,
        device: Device,
        queue: Queue,
        format: TextureFormat,
    ) -> Result<Self, Error> {
        let info = adapter.get_info();
        let (terrain_pipeline, water_pipeline, uniform, bind_group) =
            scoped(&device, "pipeline creation", || pipelines(&device, format))?;
        Ok(Self {
            device,
            queue,
            adapter_name: info.name,
            backend: info.backend,
            format,
            terrain_pipeline,
            water_pipeline,
            uniform,
            bind_group,
            depth: None,
            scene: None,
        })
    }

    /// A renderer for `format` on any adapter, without a surface.
    ///
    /// # Errors
    /// As [`Renderer::with_backends`].
    pub fn new(format: TextureFormat) -> Result<Self, Error> {
        Self::with_backends(Backends::all(), format)
    }

    /// A renderer for `format` on an adapter among `backends`.
    ///
    /// # Errors
    /// [`Error::NoAdapter`] when no adapter exists among `backends`,
    /// [`Error::DeviceRequest`] when the adapter refuses the device, and
    /// [`Error::Gpu`] when wgpu reports an error while creating the
    /// pipelines.
    pub fn with_backends(backends: Backends, format: TextureFormat) -> Result<Self, Error> {
        let mut descriptor = InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        let instance = Instance::new(descriptor);
        let (adapter, device, queue) = pollster::block_on(open(&instance, None))?;
        Self::from_parts(&adapter, device, queue, format)
    }

    /// A renderer for a window's surface: creates the surface from
    /// `target`, requests an adapter compatible with it, and picks the
    /// surface's format, preferring an sRGB one. `display` is wgpu's display
    /// handle, which GLES needs to present.
    ///
    /// # Errors
    /// [`Error::Surface`] when the surface cannot be created or offers no
    /// format, [`Error::NoAdapter`] when no adapter is compatible with it,
    /// [`Error::DeviceRequest`] when the adapter refuses the device, and
    /// [`Error::Gpu`] when wgpu reports an error while creating the
    /// pipelines.
    pub fn for_surface(
        target: impl Into<SurfaceTarget<'static>>,
        display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>,
    ) -> Result<SurfaceRenderer, Error> {
        let descriptor = match display {
            Some(handle) => InstanceDescriptor::new_with_display_handle(handle),
            None => InstanceDescriptor::new_without_display_handle(),
        };
        let instance = Instance::new(descriptor);
        let surface = instance
            .create_surface(target)
            .map_err(|error| Error::Surface {
                reason: error.to_string(),
            })?;
        let (adapter, device, queue) = pollster::block_on(open(&instance, Some(&surface)))?;
        let formats = surface.get_capabilities(&adapter).formats;
        let format = formats
            .iter()
            .copied()
            .find(TextureFormat::is_srgb)
            .or_else(|| formats.first().copied())
            .ok_or_else(|| Error::Surface {
                reason: "the surface offers no texture format".to_owned(),
            })?;
        let renderer = Self::from_parts(&adapter, device, queue, format)?;
        Ok(SurfaceRenderer {
            renderer,
            surface,
            format,
        })
    }

    /// Runs `make`, a caller's wgpu calls, inside the renderer's validation
    /// and out-of-memory error scopes, so an error they raise comes back as
    /// a value and never reaches the device's default handler, which panics.
    ///
    /// # Errors
    /// [`Error::Gpu`], naming `during`, for the first error either scope
    /// captured.
    pub fn guarded<T>(&self, during: &'static str, make: impl FnOnce() -> T) -> Result<T, Error> {
        let _ = (&self.device, make);
        Err(Error::Gpu {
            during,
            reason: "the calls were not run".to_owned(),
        })
    }

    /// The device; a caller configures its surface with this one.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// The queue that submits the renderer's work.
    #[must_use]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// The format of the views [`Renderer::draw`] accepts.
    #[must_use]
    pub fn format(&self) -> TextureFormat {
        self.format
    }

    /// The adapter's name.
    #[must_use]
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// The adapter's backend.
    #[must_use]
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// Runs the fit check against the device's buffer limit, then uploads
    /// `mesh` and `water` for the draws that follow. An upload that is
    /// refused or fails leaves the scene uploaded before it, still drawn.
    ///
    /// # Errors
    /// [`Error::GridTooLarge`] when the mesh's grid exceeds the device's
    /// maximum buffer size, [`Error::InconsistentMesh`] when its buffers do
    /// not match its cell counts, and [`Error::Gpu`] when wgpu reports an
    /// error while creating the buffers.
    pub fn upload(&mut self, mesh: &TerrainMesh, water: Option<&WaterPlane>) -> Result<(), Error> {
        grid_fits(
            mesh.columns,
            mesh.rows,
            self.device.limits().max_buffer_size,
        )?;
        let vertex_count = grid_vertex_count(mesh.columns, mesh.rows)?;
        let cells = u64::from(mesh.columns) * u64::from(mesh.rows);
        let consistent = u64::try_from(mesh.positions.len()) == Ok(u64::from(vertex_count))
            && mesh.normals.len() == mesh.positions.len()
            && u64::try_from(mesh.indices.len()) == Ok(cells * 6);
        if !consistent {
            return Err(Error::InconsistentMesh);
        }
        let vertices: Vec<f32> = mesh
            .positions
            .iter()
            .zip(&mesh.normals)
            .flat_map(|(p, n)| [p[0], p[1], p[2], n[0], n[1], n[2]])
            .collect();
        let plane = water.map(|plane| {
            let vertices: Vec<f32> = plane
                .corners
                .iter()
                .flat_map(|c| [c[0], c[1], c[2], 0.0, 1.0, 0.0])
                .collect();
            (vertices, [0, 1, 2, 0, 2, 3])
        });
        let scene = scoped(&self.device, "mesh upload", || Scene {
            terrain: self.mesh_buffers("terrain", &vertices, &mesh.indices),
            water: plane
                .as_ref()
                .map(|(vertices, indices)| self.mesh_buffers("water", vertices, indices)),
            range: [mesh.lowest, mesh.highest],
        })?;
        self.scene = Some(scene);
        Ok(())
    }

    fn mesh_buffers(&self, label: &'static str, vertices: &[f32], indices: &[u32]) -> GpuMesh {
        let make = |usage: BufferUsages, bytes: &[u8]| {
            let buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: bytes.len() as u64,
                usage: usage | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&buffer, 0, bytes);
            buffer
        };
        GpuMesh {
            vertices: make(BufferUsages::VERTEX, bytemuck::cast_slice(vertices)),
            indices: make(BufferUsages::INDEX, bytemuck::cast_slice(indices)),
            index_count: u32::try_from(indices.len()).unwrap_or(u32::MAX),
        }
    }

    /// Draws one frame for `camera` into `view`, the only draw path: the
    /// camera's view-projection matrix is used as given. `view` must be a
    /// view of a texture of the renderer's format and of `size`.
    ///
    /// # Errors
    /// [`Error::ZeroTarget`] when `size` has a zero dimension,
    /// [`Error::NothingUploaded`] before the first upload, and
    /// [`Error::Gpu`] when wgpu reports an error while recording or
    /// submitting the frame.
    pub fn draw(
        &mut self,
        camera: &Camera,
        view: &TextureView,
        size: (u32, u32),
    ) -> Result<(), Error> {
        if size.0 == 0 || size.1 == 0 {
            return Err(Error::ZeroTarget {
                width: size.0,
                height: size.1,
            });
        }
        let Some(scene) = &self.scene else {
            return Err(Error::NothingUploaded);
        };
        let stale = self.depth.as_ref().is_none_or(|depth| depth.size != size);
        let device = &self.device;
        let new_depth = if stale {
            Some(scoped(device, "depth target creation", || {
                depth_target(device, size)
            })?)
        } else {
            None
        };
        if let Some(depth) = new_depth {
            self.depth = Some(depth);
        }
        let Some(depth) = &self.depth else {
            return Err(Error::NothingUploaded);
        };
        let mut uniform = [0.0_f32; UNIFORM_FLOATS];
        for (slot, value) in uniform
            .iter_mut()
            .zip(camera.view_projection().iter().flatten())
        {
            *slot = *value;
        }
        uniform[16] = scene.range[0];
        uniform[17] = scene.range[1];
        scoped(device, "frame submission", || {
            self.queue
                .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
            let [r, g, b] = shader::CLEAR_COLOR.map(f64::from);
            {
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                        view: &depth.view,
                        depth_ops: Some(Operations {
                            load: LoadOp::Clear(1.0),
                            store: StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    ..RenderPassDescriptor::default()
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_pipeline(&self.terrain_pipeline);
                draw_mesh(&mut pass, &scene.terrain);
                if let Some(water) = &scene.water {
                    pass.set_pipeline(&self.water_pipeline);
                    draw_mesh(&mut pass, water);
                }
            }
            self.queue.submit([encoder.finish()]);
        })
    }
}

fn draw_mesh(pass: &mut wgpu::RenderPass<'_>, mesh: &GpuMesh) {
    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
    pass.set_index_buffer(mesh.indices.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
}

fn depth_target(device: &Device, size: (u32, u32)) -> DepthTarget {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("depth"),
        size: Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    DepthTarget {
        size,
        view: texture.create_view(&TextureViewDescriptor::default()),
    }
}

/// The two pipelines, their uniform buffer, and the bind group that binds it.
fn pipelines(
    device: &Device,
    format: TextureFormat,
) -> (RenderPipeline, RenderPipeline, Buffer, BindGroup) {
    let uniform_bytes = (UNIFORM_FLOATS * 4) as u64;
    let uniform = device.create_buffer(&BufferDescriptor {
        label: Some("frame uniform"),
        size: uniform_bytes,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("frame"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(uniform_bytes),
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("frame"),
        layout: &group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("first light"),
        bind_group_layouts: &[Some(&group_layout)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("first light"),
        source: ShaderSource::Wgsl(shader::shader_source().into()),
    });
    let buffers = [Some(VertexBufferLayout {
        array_stride: VERTEX_BYTES,
        step_mode: VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRIBUTES,
    })];
    let build = |label: &'static str, vertex: &'static str, fragment: &'static str| {
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: VertexState {
                module: &module,
                entry_point: Some(vertex),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &buffers,
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                ..PrimitiveState::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &module,
                entry_point: Some(fragment),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    (
        build("terrain", "vs_terrain", "fs_terrain"),
        build("water", "vs_water", "fs_water"),
        uniform,
        bind_group,
    )
}
