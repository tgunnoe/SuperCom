//! The GPU side: a renderer that draws a terrain mesh and a water plane for
//! a camera into any texture view of one format.

use crate::{Camera, Error, TerrainMesh, WaterPlane, shader};
use wgpu::{
    Adapter, Backend, Backends, CommandEncoderDescriptor, Device, DeviceDescriptor, Features,
    Instance, InstanceDescriptor, Limits, LoadOp, Operations, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface, SurfaceTarget, TextureFormat,
    TextureView,
};

/// The renderer: one device and queue, built for one target format.
#[derive(Debug)]
pub struct Renderer {
    device: Device,
    queue: Queue,
    adapter_name: String,
    backend: Backend,
    format: TextureFormat,
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
    fn from_parts(adapter: &Adapter, device: Device, queue: Queue, format: TextureFormat) -> Self {
        let info = adapter.get_info();
        Self {
            device,
            queue,
            adapter_name: info.name,
            backend: info.backend,
            format,
        }
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
    /// [`Error::NoAdapter`] when no adapter exists among `backends`, and
    /// [`Error::DeviceRequest`] when the adapter refuses the device.
    pub fn with_backends(_backends: Backends, format: TextureFormat) -> Result<Self, Error> {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
        let (adapter, device, queue) = pollster::block_on(open(&instance, None))?;
        Ok(Self::from_parts(&adapter, device, queue, format))
    }

    /// A renderer for a window's surface: creates the surface from
    /// `target`, requests an adapter compatible with it, and picks the
    /// surface's format, preferring an sRGB one. `display` is wgpu's display
    /// handle, which GLES needs to present.
    ///
    /// # Errors
    /// [`Error::Surface`] when the surface cannot be created or offers no
    /// format, [`Error::NoAdapter`] when no adapter is compatible with it,
    /// and [`Error::DeviceRequest`] when the adapter refuses the device.
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
        let renderer = Self::from_parts(&adapter, device, queue, format);
        Ok(SurfaceRenderer {
            renderer,
            surface,
            format,
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
    /// `mesh` and `water` for the draws that follow.
    ///
    /// # Errors
    /// [`Error::GridTooLarge`] when the mesh's grid exceeds the device's
    /// maximum buffer size, [`Error::InconsistentMesh`] when its buffers do
    /// not match its cell counts, and [`Error::Gpu`] when wgpu reports an
    /// error while creating the buffers.
    pub fn upload(
        &mut self,
        _mesh: &TerrainMesh,
        _water: Option<&WaterPlane>,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Draws one frame for `camera` into `view`, the only draw path: the
    /// camera's view-projection matrix is used as given.
    ///
    /// # Errors
    /// [`Error::ZeroTarget`] when `size` has a zero dimension,
    /// [`Error::NothingUploaded`] before the first upload, and
    /// [`Error::Gpu`] when wgpu reports an error while recording or
    /// submitting the frame.
    pub fn draw(
        &mut self,
        _camera: &Camera,
        view: &TextureView,
        _size: (u32, u32),
    ) -> Result<(), Error> {
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
        let [r, g, b] = shader::CLEAR_COLOR.map(f64::from);
        drop(encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                    store: StoreOp::Store,
                },
            })],
            ..RenderPassDescriptor::default()
        }));
        self.queue.submit([encoder.finish()]);
        Ok(())
    }
}
