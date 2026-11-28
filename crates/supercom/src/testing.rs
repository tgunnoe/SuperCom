//! Helpers the binary's unit tests share: scratch directories under the
//! system's temporary directory, and the committed log's path.

use crate::cache::CacheVars;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh directory under the system's temporary directory, removed on
/// drop.
pub struct Scratch(PathBuf);

impl Scratch {
    /// Makes the directory, named for `label`, the process, and a counter.
    pub fn new(label: &str) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("supercom-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the scratch directory is created");
        Self(dir)
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The cache variables that name a cache directory inside this one.
    pub fn cache_vars(&self) -> CacheVars {
        CacheVars {
            override_dir: Some(self.0.join("cache").into_os_string()),
            ..CacheVars::default()
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The committed log, by a path built from the manifest's directory.
pub fn committed_log() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../replay-harness/fixtures/skirmish_scmp_012.replay")
}

/// Draws one frame of `renderer` for `camera` into a fresh offscreen square
/// of `size` pixels and reads it back, row 0 at the top.
pub fn draw_offscreen(
    renderer: &mut supercom_render::Renderer,
    camera: &supercom_render::Camera,
    size: u32,
) -> Vec<[u8; 3]> {
    use supercom_render::wgpu::{
        BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, MapMode, Origin3d,
        PollType, TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
        TextureDescriptor, TextureDimension, TextureUsages, TextureViewDescriptor,
    };
    let device = renderer.device().clone();
    let extent = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("offscreen"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: renderer.format(),
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    renderer
        .draw(camera, &view, (size, size))
        .expect("the draw accepts a well-formed target");
    let row_bytes = (size * 4).next_multiple_of(256);
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row_bytes) * u64::from(size),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(size),
            },
        },
        extent,
    );
    renderer.queue().submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.map_async(MapMode::Read, .., move |result| {
        drop(sender.send(result));
    });
    device
        .poll(PollType::wait_indefinitely())
        .expect("the device finishes the copy");
    receiver
        .recv()
        .expect("the map callback runs")
        .expect("the readback buffer maps");
    let bytes = buffer.get_mapped_range(..).expect("the buffer is mapped");
    let mut pixels = Vec::new();
    for y in 0..size {
        let start = usize::try_from(y * row_bytes).expect("a row offset fits usize");
        for x in 0..size {
            let at = start + usize::try_from(x).expect("a column fits usize") * 4;
            pixels.push([bytes[at], bytes[at + 1], bytes[at + 2]]);
        }
    }
    pixels
}
