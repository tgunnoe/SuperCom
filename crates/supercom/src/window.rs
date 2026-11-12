//! The window loop: converts the window system's events into inputs, calls
//! the pure functions of the other modules, and hands each frame to the
//! renderer's one draw function. It holds no arithmetic of its own, and no
//! headless test reaches it.

use crate::error::RunError;
use crate::frame::{FramePlan, Progress, Step, frame_plan};
use crate::input::{Flow, Input, apply_input, key_input, wheel_input};
use std::num::NonZeroU32;
use std::sync::Arc;
use supercom_render::wgpu::CurrentSurfaceTexture;
use supercom_render::wgpu::wgt::WgpuHasDisplayHandle;
use supercom_render::wgpu::{
    CompositeAlphaMode, PresentMode, SurfaceColorSpace, SurfaceConfiguration, SurfaceTexture,
    TextureUsages, TextureView, TextureViewDescriptor,
};
use supercom_render::{Camera, Renderer, SurfaceRenderer, TerrainMesh, WaterPlane, grid_fits};
use supercom_scmap::Map;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

/// How a run of the window ended without failing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The requested number of frames was presented.
    Presented(u32),
    /// The window was closed.
    Closed,
}

/// What acquiring the frame's texture came to.
enum Acquired {
    Frame(SurfaceTexture, TextureView),
    Reconfigure,
    Occluded,
    Retry,
}

/// The window and everything drawn into it.
struct Gfx {
    window: Arc<Window>,
    surface: SurfaceRenderer,
    camera: Camera,
    configured: Option<(u32, u32)>,
    aspect_reported: bool,
}

struct App {
    map: Map,
    progress: Progress,
    gfx: Option<Gfx>,
    shift: bool,
    outcome: Option<Result<Outcome, RunError>>,
}

/// Opens a window on `map` and runs until it closes, `frames` frames have
/// been presented, or a step fails.
///
/// # Errors
/// [`RunError`] for the first step that fails.
pub fn run(
    event_loop: EventLoop<()>,
    map: Map,
    frames: Option<NonZeroU32>,
) -> Result<Outcome, RunError> {
    let mut app = App {
        map,
        progress: Progress::new(frames),
        gfx: None,
        shift: false,
        outcome: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| RunError::Window {
            reason: error.to_string(),
        })?;
    app.outcome.unwrap_or(Ok(Outcome::Closed))
}

/// Everything between the window and the first frame, in the order of the
/// run: the window, the renderer for its surface, the grid fit check against
/// that renderer's device, the mesh, the water, the camera, the upload.
fn build(event_loop: &ActiveEventLoop, map: &Map) -> Result<Gfx, RunError> {
    let attributes = Window::default_attributes()
        .with_title("SuperCom")
        .with_inner_size(LogicalSize::new(1280.0, 720.0));
    let window =
        Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|error| RunError::Window {
                    reason: error.to_string(),
                })?,
        );
    let display: Option<Box<dyn WgpuHasDisplayHandle>> =
        Some(Box::new(event_loop.owned_display_handle()));
    let mut surface = Renderer::for_surface(Arc::clone(&window), display)?;
    eprintln!(
        "adapter: {} ({:?}), format {:?}",
        surface.renderer.adapter_name(),
        surface.renderer.backend(),
        surface.format
    );
    grid_fits(
        map.heightmap.width,
        map.heightmap.height,
        surface.renderer.device().limits().max_buffer_size,
    )?;
    let mesh = TerrainMesh::build(map)?;
    let water = WaterPlane::from_map(map);
    let size = window.inner_size();
    let camera = Camera::for_mesh(&mesh, crate::frame::aspect((size.width, size.height)))?;
    surface.renderer.upload(&mesh, water.as_ref())?;
    Ok(Gfx {
        window,
        surface,
        camera,
        configured: None,
        aspect_reported: false,
    })
}

impl App {
    fn finish(&mut self, event_loop: &ActiveEventLoop, result: Result<Outcome, RunError>) {
        if self.outcome.is_none() {
            self.outcome = Some(result);
        }
        event_loop.exit();
    }

    fn input(&mut self, event_loop: &ActiveEventLoop, input: Input) {
        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        match apply_input(&mut gfx.camera, input) {
            Flow::Close => self.finish(event_loop, Ok(Outcome::Closed)),
            Flow::Continue => gfx.window.request_redraw(),
        }
    }

    /// Counts a frame that did not present, or ends the run after too many.
    fn stall(&mut self, event_loop: &ActiveEventLoop) {
        match self.progress.stalled() {
            Step::GiveUp => self.finish(
                event_loop,
                Err(RunError::Window {
                    reason: "the surface did not present, frame after frame".to_owned(),
                }),
            ),
            Step::Continue | Step::Done => {
                if let Some(gfx) = &self.gfx {
                    gfx.window.request_redraw();
                }
            }
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        let inner = gfx.window.inner_size();
        let size = (inner.width, inner.height);
        let FramePlan::Draw { configure, aspect } = frame_plan(size, gfx.configured) else {
            return;
        };
        if configure {
            if let Err(error) = gfx.camera.set_aspect(aspect)
                && !gfx.aspect_reported
            {
                gfx.aspect_reported = true;
                eprintln!("supercom: the camera keeps its aspect ratio: {error}");
            }
            let config = SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: gfx.surface.format,
                color_space: SurfaceColorSpace::Auto,
                width: size.0,
                height: size.1,
                present_mode: PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: CompositeAlphaMode::Auto,
                view_formats: Vec::new(),
            };
            let surface = &gfx.surface;
            let done = surface.renderer.guarded("surface configuration", || {
                surface
                    .surface
                    .configure(surface.renderer.device(), &config);
            });
            match done {
                Ok(()) => gfx.configured = Some(size),
                Err(error) => return self.finish(event_loop, Err(error.into())),
            }
        }
        let surface = &gfx.surface;
        let acquired = surface.renderer.guarded("frame acquisition", || {
            match surface.surface.get_current_texture() {
                CurrentSurfaceTexture::Success(frame)
                | CurrentSurfaceTexture::Suboptimal(frame) => {
                    let view = frame.texture.create_view(&TextureViewDescriptor::default());
                    Acquired::Frame(frame, view)
                }
                CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Lost => {
                    Acquired::Reconfigure
                }
                CurrentSurfaceTexture::Occluded => Acquired::Occluded,
                _ => Acquired::Retry,
            }
        });
        match acquired {
            Err(error) => self.finish(event_loop, Err(error.into())),
            Ok(Acquired::Occluded) => {}
            Ok(Acquired::Retry) => self.stall(event_loop),
            Ok(Acquired::Reconfigure) => {
                gfx.configured = None;
                self.stall(event_loop);
            }
            Ok(Acquired::Frame(frame, view)) => {
                let drawn = gfx.surface.renderer.draw(&gfx.camera, &view, size);
                let surface = &gfx.surface;
                let shown = drawn.and_then(|()| {
                    surface
                        .renderer
                        .guarded("presentation", || surface.renderer.queue().present(frame))
                });
                if let Err(error) = shown {
                    return self.finish(event_loop, Err(error.into()));
                }
                match self.progress.presented() {
                    Step::Done => {
                        let count = self.progress.count();
                        self.finish(event_loop, Ok(Outcome::Presented(count)));
                    }
                    Step::Continue | Step::GiveUp => gfx.window.request_redraw(),
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        match build(event_loop, &self.map) {
            Ok(gfx) => {
                gfx.window.request_redraw();
                self.gfx = Some(gfx);
            }
            Err(error) => self.finish(event_loop, Err(error)),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.finish(event_loop, Ok(Outcome::Closed)),
            WindowEvent::Resized(_) | WindowEvent::Occluded(false) => {
                if let Some(gfx) = &self.gfx {
                    gfx.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.shift = modifiers.state().shift_key();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.input(event_loop, wheel_input(delta, self.shift));
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(input) = key_input(code)
                {
                    self.input(event_loop, input);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}
