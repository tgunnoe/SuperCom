use super::*;
use supercom_render::ZoomSpeed;
use winit::dpi::PhysicalPosition;

const ASPECT: f32 = 16.0 / 9.0;

fn camera() -> Camera {
    Camera::new(4096.0, 4096.0, 0.0, 50.0, ASPECT).expect("the camera builds")
}

/// A camera at the near end of its zoom range, where the whole extent is
/// open to the target.
fn near_camera() -> Camera {
    let mut camera = camera();
    camera.zoom(-1000.0, ZoomSpeed::Normal);
    camera
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn wheel(lines: f32, fast: bool) -> Input {
    Input::Wheel { lines, fast }
}

/// The wheel, the pan keys, the strategic key, and Escape.
#[test]
fn supercom_input_drives_the_camera() {
    // One line away from the user is one step toward the near end, at 0.02
    // of the zoom range, or 0.08 with shift held.
    let mut c = camera();
    let (near_end, far) = c.zoom_range();
    let range = far - near_end;
    let tol = range * 1e-4;
    assert_eq!(apply_input(&mut c, wheel(1.0, false)), Flow::Continue);
    assert!(
        near(c.distance(), far - 0.02 * range, tol),
        "{}",
        c.distance()
    );
    apply_input(&mut c, wheel(-1.0, false));
    assert!(
        near(c.distance(), far, tol),
        "a line toward the user zooms out"
    );
    apply_input(&mut c, wheel(1.0, true));
    assert!(
        near(c.distance(), far - 0.08 * range, tol),
        "{}",
        c.distance()
    );
    let mut c = camera();
    apply_input(&mut c, wheel(2.5, false));
    assert!(
        near(c.distance(), far - 0.05 * range, tol),
        "{}",
        c.distance()
    );

    // Input that is not finite changes nothing.
    let before = c;
    apply_input(&mut c, wheel(f32::NAN, false));
    apply_input(&mut c, wheel(f32::INFINITY, true));
    assert_eq!(c, before);

    // A pixel-delta wheel is 40 pixels to the line, fractionally.
    assert_eq!(
        wheel_input(
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 40.0)),
            false
        ),
        wheel(1.0, false)
    );
    assert_eq!(
        wheel_input(
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -10.0)),
            true
        ),
        wheel(-0.25, true)
    );
    assert_eq!(
        wheel_input(MouseScrollDelta::LineDelta(0.0, 2.0), false),
        wheel(2.0, false)
    );
    assert_eq!(
        wheel_input(MouseScrollDelta::LineDelta(3.0, 0.0), false),
        wheel(0.0, false),
        "only the vertical delta zooms"
    );

    // Each pan key moves the target by 0.04 of the distance.
    let mut c = near_camera();
    let step = 0.04 * c.distance();
    let start = c.target();
    for (direction, dx, dz) in [
        (Direction::East, step, 0.0),
        (Direction::West, -step, 0.0),
        (Direction::South, 0.0, step),
        (Direction::North, 0.0, -step),
    ] {
        let from = c.target();
        assert_eq!(apply_input(&mut c, Input::Pan(direction)), Flow::Continue);
        let to = c.target();
        assert!(
            near(to[0] - from[0], dx, 1e-3) && near(to[2] - from[2], dz, 1e-3),
            "{direction:?} moved by ({}, {})",
            to[0] - from[0],
            to[2] - from[2]
        );
    }
    assert!(near(c.target()[0], start[0], 1e-2) && near(c.target()[2], start[2], 1e-2));

    // The keys: WASD and the arrows pan, Home is the strategic view, Escape
    // closes, and any other key is nothing.
    for (code, input) in [
        (KeyCode::KeyW, Input::Pan(Direction::North)),
        (KeyCode::ArrowUp, Input::Pan(Direction::North)),
        (KeyCode::KeyS, Input::Pan(Direction::South)),
        (KeyCode::ArrowDown, Input::Pan(Direction::South)),
        (KeyCode::KeyA, Input::Pan(Direction::West)),
        (KeyCode::ArrowLeft, Input::Pan(Direction::West)),
        (KeyCode::KeyD, Input::Pan(Direction::East)),
        (KeyCode::ArrowRight, Input::Pan(Direction::East)),
        (KeyCode::Home, Input::Strategic),
        (KeyCode::Escape, Input::Close),
    ] {
        assert_eq!(key_input(code), Some(input), "{code:?}");
    }
    assert_eq!(key_input(KeyCode::KeyQ), None);

    // The strategic key returns to the far end over the map's centre.
    let mut c = near_camera();
    apply_input(&mut c, Input::Pan(Direction::East));
    apply_input(&mut c, Input::Pan(Direction::South));
    assert_eq!(apply_input(&mut c, Input::Strategic), Flow::Continue);
    assert!(near(c.distance(), c.zoom_range().1, 1e-3));
    assert!(near(c.target()[0], 2048.0, 1e-3) && near(c.target()[2], 2048.0, 1e-3));

    // Escape closes and leaves the camera as it was.
    let mut c = near_camera();
    let before = c;
    assert_eq!(apply_input(&mut c, Input::Close), Flow::Close);
    assert_eq!(c, before);
}
