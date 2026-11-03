//! Tests for `default-floating-position` with `relative-to="cursor"`.
//!
//! These run against the real Wayland server and a real client: the pointer is moved by
//! injecting a motion into the seat (the same state that the input handlers leave behind) and the
//! resulting window position is read back from the layout's IPC geometry.

use niri_config::{Action, Config};
use smithay::input::pointer::MotionEvent;
use smithay::utils::{Point, SERIAL_COUNTER};

use super::client::ClientId;
use super::fixture::Fixture;

const OUTPUT_W: u16 = 1920;
const OUTPUT_H: u16 = 1080;

const WINDOW_W: u16 = 100;
const WINDOW_H: u16 = 100;

/// Offset in the cursor rules below.
const OFFSET_X: f64 = 40.;
const OFFSET_Y: f64 = 30.;

/// Position used when there is no pointer to place the window relative to: the default centered
/// position of a 100x100 tile in a 1920x1080 working area.
fn centered_tile_pos() -> (f64, f64) {
    let x = (f64::from(OUTPUT_W) - f64::from(WINDOW_W)) / 2.;
    let y = (f64::from(OUTPUT_H) - f64::from(WINDOW_H)) / 2.;
    (x, y)
}

/// Opens windows floating with the given `default-floating-position`.
const CONFIG_CURSOR: &str = r#"
animations {
    off
}

layout {
    border {
        off
    }
}

window-rule {
    open-floating true
    default-floating-position x=40 y=30 relative-to="cursor"
}
"#;

/// Same cursor position, but windows open tiled so that they can be toggled to floating.
const CONFIG_CURSOR_TOGGLE: &str = r#"
animations {
    off
}

layout {
    border {
        off
    }
}

window-rule {
    default-floating-position x=40 y=30 relative-to="cursor"
}
"#;

fn parse_config(text: &str) -> Config {
    Config::parse_mem(text).unwrap()
}

fn set_up(text: &str) -> (Fixture, ClientId) {
    let mut f = Fixture::with_config(parse_config(text));
    f.add_output(1, (OUTPUT_W, OUTPUT_H));
    let id = f.add_client();
    (f, id)
}

/// Moves the pointer to a global logical location, as an input device would.
fn set_pointer_location(f: &mut Fixture, x: f64, y: f64) {
    let event = MotionEvent {
        location: Point::from((x, y)),
        serial: SERIAL_COUNTER.next_serial(),
        time: 0,
    };

    let pointer = f.niri().seat.get_pointer().unwrap();
    pointer.motion(f.niri_state(), None, &event);
    pointer.frame(f.niri_state());
}

/// Creates a 100×100 window and maps it.
///
/// `between_configure_and_map` runs after the initial configure roundtrip but before the attach
/// that maps the window, i.e. immediately before the map.
fn map_window(f: &mut Fixture, id: ClientId, between_configure_and_map: impl FnOnce(&mut Fixture)) {
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);

    between_configure_and_map(f);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(WINDOW_W, WINDOW_H);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
}

/// Returns the workspace-view position of the only floating window, in logical pixels.
fn floating_tile_pos(f: &mut Fixture) -> (f64, f64) {
    let workspace = f.niri().layout.active_workspace().unwrap();
    let mut layouts = workspace.floating().tiles_with_ipc_layouts();
    let (_, layout) = layouts.next().unwrap();
    assert!(
        layouts.next().is_none(),
        "expected exactly one floating window"
    );
    layout.tile_pos_in_workspace_view.unwrap()
}

fn assert_pos(actual: (f64, f64), expected: (f64, f64)) {
    let close = (actual.0 - expected.0).abs() < 0.01 && (actual.1 - expected.1).abs() < 0.01;
    assert!(close, "expected position {expected:?}, got {actual:?}");
}

#[test]
fn cursor_position_used_for_new_floating_window() {
    let (mut f, id) = set_up(CONFIG_CURSOR);

    set_pointer_location(&mut f, 500., 400.);
    map_window(&mut f, id, |_| {});

    assert_pos(
        floating_tile_pos(&mut f),
        (500. + OFFSET_X, 400. + OFFSET_Y),
    );
}

#[test]
fn pointer_sampled_at_map_not_at_initial_configure() {
    let (mut f, id) = set_up(CONFIG_CURSOR);

    // The pointer is here when the client first commits and gets its initial configure.
    set_pointer_location(&mut f, 300., 200.);

    // Move the pointer after the initial configure but before the window is mapped. The window
    // must open at the map-time pointer position.
    map_window(&mut f, id, |f| set_pointer_location(f, 500., 400.));

    assert_pos(
        floating_tile_pos(&mut f),
        (500. + OFFSET_X, 400. + OFFSET_Y),
    );
}

#[test]
fn toggle_floating_action_samples_pointer_at_action_time() {
    let (mut f, id) = set_up(CONFIG_CURSOR_TOGGLE);

    set_pointer_location(&mut f, 300., 200.);
    map_window(&mut f, id, |_| {});

    // The window is tiled, so there is nothing in the floating layout yet.
    assert!(f
        .niri()
        .layout
        .active_workspace()
        .unwrap()
        .floating()
        .tiles_with_ipc_layouts()
        .next()
        .is_none());

    set_pointer_location(&mut f, 600., 500.);
    f.niri_state()
        .do_action(Action::ToggleWindowFloating, false);
    f.double_roundtrip(id);

    assert_pos(
        floating_tile_pos(&mut f),
        (600. + OFFSET_X, 500. + OFFSET_Y),
    );
}

#[test]
fn stored_floating_position_wins_over_later_pointer_position() {
    let (mut f, id) = set_up(CONFIG_CURSOR);

    map_window(&mut f, id, |f| set_pointer_location(f, 500., 400.));
    assert_pos(
        floating_tile_pos(&mut f),
        (500. + OFFSET_X, 400. + OFFSET_Y),
    );

    // Move the pointer elsewhere and toggle the window out of and back into the floating layout.
    set_pointer_location(&mut f, 200., 200.);
    f.niri_state()
        .do_action(Action::ToggleWindowFloating, false);
    f.niri_state()
        .do_action(Action::ToggleWindowFloating, false);
    f.double_roundtrip(id);

    // The stored floating position wins over the current pointer position.
    assert_pos(
        floating_tile_pos(&mut f),
        (500. + OFFSET_X, 400. + OFFSET_Y),
    );
}

#[test]
fn cursor_position_clamped_within_working_area() {
    let (mut f, id) = set_up(CONFIG_CURSOR);

    // The window would go past the bottom-right corner of the working area.
    set_pointer_location(&mut f, 1900., 1070.);
    map_window(&mut f, id, |_| {});

    let expected_x = f64::from(OUTPUT_W) - f64::from(WINDOW_W);
    let expected_y = f64::from(OUTPUT_H) - f64::from(WINDOW_H);
    assert_pos(floating_tile_pos(&mut f), (expected_x, expected_y));
}

#[test]
fn cursor_position_falls_back_when_pointer_is_off_output() {
    let (mut f, id) = set_up(CONFIG_CURSOR);

    set_pointer_location(&mut f, -500., -500.);
    map_window(&mut f, id, |_| {});

    // With no pointer position available, the cursor rule falls back to the default centered
    // position.
    assert_pos(floating_tile_pos(&mut f), centered_tile_pos());
}

#[test]
fn cursor_position_falls_back_when_pointer_is_on_another_output() {
    let mut f = Fixture::with_config(parse_config(CONFIG_CURSOR));
    f.add_output(1, (OUTPUT_W, OUTPUT_H));
    f.add_output(2, (1280, 720));
    f.niri_focus_output(1);
    let id = f.add_client();

    // The pointer is on the second output while the window opens on the first one, so the first
    // output's workspace has no pointer position and falls back to the centered position.
    set_pointer_location(&mut f, f64::from(OUTPUT_W) + 100., 100.);
    map_window(&mut f, id, |_| {});

    assert_pos(floating_tile_pos(&mut f), centered_tile_pos());
}

#[test]
fn cursor_variant_does_not_change_anchor_placement() {
    let (mut f, id) = set_up(
        r#"
animations {
    off
}

layout {
    border {
        off
    }
}

window-rule {
    open-floating true
    default-floating-position x=40 y=30 relative-to="bottom-right"
}
"#,
    );

    set_pointer_location(&mut f, 500., 400.);
    map_window(&mut f, id, |_| {});

    let expected_x = f64::from(OUTPUT_W) - f64::from(WINDOW_W) - OFFSET_X;
    let expected_y = f64::from(OUTPUT_H) - f64::from(WINDOW_H) - OFFSET_Y;
    assert_pos(floating_tile_pos(&mut f), (expected_x, expected_y));
}
