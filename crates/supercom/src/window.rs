//! The window loop: converts the window system's events into inputs, calls
//! the pure functions of the other modules, and hands each frame to the
//! renderer's one draw function. It holds no arithmetic of its own; the one
//! owner of a replay's state is here, [`Play`], and its stepping is tested
//! without a window.

use crate::error::RunError;
use crate::frame::{FramePlan, Progress, Step, frame_plan};
use crate::input::{Flow, Input, apply_input, key_input, wheel_input};
use crate::interp::Kept;
use crate::pacing::{Pace, pace};
use crate::prepare::Prepared;
use crate::scene::scene_instances;
use crate::stepping::{Verdict, may_step, summary_line, verdict};
use crate::units::Meshes;
use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use supercom_render::wgpu::CurrentSurfaceTexture;
use supercom_render::wgpu::wgt::WgpuHasDisplayHandle;
use supercom_render::wgpu::{
    CompositeAlphaMode, PresentMode, SurfaceColorSpace, SurfaceConfiguration, SurfaceTexture,
    TextureUsages, TextureView, TextureViewDescriptor,
};
use supercom_render::{Camera, Renderer, SurfaceRenderer, TerrainMesh, WaterPlane, grid_fits};
use supercom_scmap::Map;
use supercom_sim::{Digest, Replay, State, commands_at};
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

/// What one step of a replay came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stepped {
    /// The step ran and returned this digest, which matched its checkpoint
    /// or had none.
    Digest(Digest),
    /// The step at the last checkpoint's tick has run; nothing more is
    /// stepped.
    Stopped,
}

/// The one owner of a replay's state, with the values kept from before the
/// last step and the run's counters.
pub struct Play {
    state: State,
    replay: Replay,
    kept: Kept,
    ticks_run: u64,
    matched: usize,
    held_frames: u64,
    meshes: Meshes,
    unnamed: BTreeSet<String>,
}

impl Play {
    /// A run at tick 0 of `state` over `replay`, with `meshes` the kinds
    /// the run resolved.
    #[must_use]
    pub fn new(state: State, replay: Replay, meshes: Meshes) -> Self {
        let mut play = Self {
            state,
            replay,
            kept: Kept::default(),
            ticks_run: 0,
            matched: 0,
            held_frames: 0,
            meshes,
            unnamed: BTreeSet::new(),
        };
        play.note_unnamed();
        play
    }

    /// Notes the kinds the state now holds that the run did not resolve.
    fn note_unnamed(&mut self) {
        self.unnamed.extend(self.meshes.unnamed(&self.state));
    }

    /// The kinds the run resolved.
    #[must_use]
    pub fn meshes(&self) -> &Meshes {
        &self.meshes
    }

    /// The run's summary line, from the loop's own counts.
    #[must_use]
    pub fn summary(&self) -> String {
        summary_line(
            self.ticks_run,
            self.matched,
            self.replay.checkpoints.len(),
            self.held_frames,
            self.unnamed.len(),
        )
    }

    /// How many distinct kinds met so far the run did not resolve.
    #[cfg(test)]
    #[must_use]
    pub fn unnamed(&self) -> usize {
        self.unnamed.len()
    }

    /// Steps one tick with the commands stamped for it, compares the digest
    /// with the replay's checkpoint, and keeps the values from before the
    /// step.
    ///
    /// # Errors
    /// [`RunError::Mismatch`] naming the tick when the digest differs from
    /// its checkpoint, and [`RunError::Step`] when the simulation refuses
    /// the step.
    pub fn step_one(&mut self) -> Result<Stepped, RunError> {
        let tick = self.state.tick;
        if !may_step(tick, &self.replay.checkpoints) {
            return Ok(Stepped::Stopped);
        }
        self.kept = Kept::of(&self.state);
        let digest = self
            .state
            .step(commands_at(&self.replay, tick))
            .map_err(|error| RunError::Step {
                tick: tick.0,
                reason: error.to_string(),
            })?;
        self.ticks_run = self.ticks_run.saturating_add(1);
        self.note_unnamed();
        match verdict(tick, &digest, &self.replay.checkpoints) {
            Verdict::Match => self.matched += 1,
            Verdict::NoCheckpoint => {}
            Verdict::Mismatch { tick } => return Err(RunError::Mismatch { tick: tick.0 }),
        }
        Ok(Stepped::Digest(digest))
    }

    /// Steps the ticks `pace` says are due, and counts the frame once when
    /// the cap held ticks back while stepping had not stopped.
    ///
    /// # Errors
    /// As [`Play::step_one`].
    pub fn frame(&mut self, pace: &Pace) -> Result<(), RunError> {
        if self.finished() {
            return Ok(());
        }
        if pace.held_back > 0 {
            self.held_frames = self.held_frames.saturating_add(1);
        }
        for _ in 0..pace.due {
            if self.step_one()? == Stepped::Stopped {
                break;
            }
        }
        Ok(())
    }

    /// Whether stepping has stopped.
    #[must_use]
    pub fn finished(&self) -> bool {
        !may_step(self.state.tick, &self.replay.checkpoints)
    }

    /// The state, for drawing.
    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    /// The values kept from before the last step.
    #[must_use]
    pub fn kept(&self) -> &Kept {
        &self.kept
    }

    /// The ticks stepped so far.
    #[must_use]
    pub fn ticks_run(&self) -> u64 {
        self.ticks_run
    }

    /// The checkpoints matched so far.
    #[cfg(test)]
    #[must_use]
    pub fn matched(&self) -> usize {
        self.matched
    }

    /// The checkpoints in the replay.
    #[cfg(test)]
    #[must_use]
    pub fn total(&self) -> usize {
        self.replay.checkpoints.len()
    }

    /// The frames at which the cap held ticks back.
    #[cfg(test)]
    #[must_use]
    pub fn held_frames(&self) -> u64 {
        self.held_frames
    }
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
    mesh: TerrainMesh,
    configured: Option<(u32, u32)>,
    aspect_reported: bool,
}

/// A replay's run inside the window: the owner of the state, the clock,
/// and whether the summary line has been written.
struct Replaying {
    play: Play,
    speed: f64,
    /// Started once the window and the renderer are built, so the time
    /// they take is never owed as ticks.
    clock: Option<Instant>,
    fraction: f64,
    reported: bool,
}

impl Replaying {
    /// Steps what the clock says is due and sets the fraction to draw at;
    /// writes the summary line when stepping has just stopped.
    fn advance(&mut self) -> Result<(), RunError> {
        let Some(start) = self.clock else {
            return Ok(());
        };
        if self.play.finished() {
            self.fraction = 1.0;
        } else {
            let owed = pace(
                start.elapsed().as_secs_f64(),
                self.speed,
                self.play.ticks_run(),
            );
            self.play.frame(&owed)?;
            self.fraction = if self.play.finished() {
                1.0
            } else {
                owed.fraction
            };
        }
        if self.play.finished() {
            self.report();
        }
        Ok(())
    }

    /// Writes the summary line to stdout, once.
    fn report(&mut self) {
        if !self.reported {
            self.reported = true;
            println!("{}", self.play.summary());
        }
    }
}

struct App {
    map: Map,
    progress: Progress,
    gfx: Option<Gfx>,
    shift: bool,
    outcome: Option<Result<Outcome, RunError>>,
    replay: Option<Replaying>,
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
        replay: None,
    };
    drive(event_loop, &mut app)
}

/// Runs `app` in `event_loop` until it finishes.
fn drive(event_loop: EventLoop<()>, app: &mut App) -> Result<Outcome, RunError> {
    event_loop.run_app(app).map_err(|error| RunError::Window {
        reason: error.to_string(),
    })?;
    app.outcome.take().unwrap_or(Ok(Outcome::Closed))
}

/// Plays `prepared` in a window at `speed` times real time until the window
/// closes, `frames` frames have been presented, or a step fails.
///
/// # Errors
/// [`RunError`] for the first step that fails.
pub fn run_replay(
    event_loop: EventLoop<()>,
    prepared: Prepared,
    speed: f64,
    frames: Option<NonZeroU32>,
) -> Result<Outcome, RunError> {
    let Prepared {
        map,
        replay,
        state,
        meshes,
    } = prepared;
    let mut app = App {
        map,
        progress: Progress::new(frames),
        gfx: None,
        shift: false,
        outcome: None,
        replay: Some(Replaying {
            play: Play::new(state, replay, meshes),
            speed,
            clock: None,
            fraction: 1.0,
            reported: false,
        }),
    };
    drive(event_loop, &mut app)
}

/// Hands the renderer the instance list of the replay's state at the
/// fraction of a tick it is drawn at.
fn set_scene(replaying: &Replaying, gfx: &mut Gfx) -> Result<(), RunError> {
    let state = replaying.play.state();
    let list = scene_instances(
        state,
        replaying.play.kept(),
        replaying.fraction,
        &state.world().units,
        replaying.play.meshes(),
        &gfx.mesh,
        gfx.camera.distance(),
    )?;
    gfx.surface.renderer.set_instances(&list)?;
    Ok(())
}

/// Everything between the window and the first frame, in the order of the
/// run: the window, the renderer for its surface, the grid fit check against
/// that renderer's device, the mesh, the water, the camera, the upload, and,
/// for a replay, the model set (the markers and one model for each resolved
/// kind).
fn build(
    event_loop: &ActiveEventLoop,
    map: &Map,
    meshes: Option<&Meshes>,
) -> Result<Gfx, RunError> {
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
    if let Some(meshes) = meshes {
        surface.renderer.upload_models(&meshes.model_set())?;
    }
    Ok(Gfx {
        window,
        surface,
        camera,
        mesh,
        configured: None,
        aspect_reported: false,
    })
}

impl App {
    fn finish(&mut self, event_loop: &ActiveEventLoop, result: Result<Outcome, RunError>) {
        if result.is_ok()
            && let Some(replaying) = self.replay.as_mut()
        {
            replaying.report();
        }
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
        if let Some(replaying) = self.replay.as_mut()
            && let Err(error) = replaying.advance()
        {
            return self.finish(event_loop, Err(error));
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
                if let Some(replaying) = &self.replay
                    && let Err(error) = set_scene(replaying, gfx)
                {
                    return self.finish(event_loop, Err(error));
                }
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
        match build(
            event_loop,
            &self.map,
            self.replay
                .as_ref()
                .map(|replaying| replaying.play.meshes()),
        ) {
            Ok(gfx) => {
                if let Some(replaying) = self.replay.as_mut() {
                    replaying.clock = Some(Instant::now());
                }
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

#[cfg(test)]
mod tests;
