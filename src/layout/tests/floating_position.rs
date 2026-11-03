//! Tests for placing new floating windows relative to the pointer.
//!
//! The layout never follows pointer motion: the compositor passes the pointer position
//! immediately before a placement operation. These tests drive that entry point directly.

use niri_config::{FloatingPosition, RelativeTo};

use super::*;

/// Size of a test window created with [`TestWindowParams::new`], in logical pixels.
const WINDOW_SIZE: (f64, f64) = (100., 200.);

fn cursor_rules(offset_x: f64, offset_y: f64) -> ResolvedWindowRules {
    position_rules(RelativeTo::Cursor, offset_x, offset_y)
}

fn position_rules(relative_to: RelativeTo, offset_x: f64, offset_y: f64) -> ResolvedWindowRules {
    ResolvedWindowRules {
        default_floating_position: Some(FloatingPosition {
            offset_x: FloatOrInt(offset_x),
            offset_y: FloatOrInt(offset_y),
            relative_to,
        }),
        ..ResolvedWindowRules::default()
    }
}

fn new_layout(outputs: usize) -> Layout<TestWindow> {
    let mut layout = Layout::default();
    for id in 1..=outputs {
        Op::AddOutput(id).apply(&mut layout);
    }
    layout.verify_invariants();
    layout
}

fn output(layout: &Layout<TestWindow>, id: usize) -> Output {
    let name = format!("output{id}");
    layout
        .outputs()
        .find(|output| output.name() == name)
        .cloned()
        .expect("output must exist")
}

fn update_pointer(layout: &mut Layout<TestWindow>, output_id: usize, pos: Point<f64, Logical>) {
    let output = output(layout, output_id);
    layout.update_pointer_position(Some((&output, pos)));
    layout.verify_invariants();
}

fn clear_pointer(layout: &mut Layout<TestWindow>) {
    layout.update_pointer_position(None);
    layout.verify_invariants();
}

fn add_window(
    layout: &mut Layout<TestWindow>,
    id: usize,
    is_floating: bool,
    rules: ResolvedWindowRules,
) {
    let params = TestWindowParams {
        is_floating,
        rules: Some(rules),
        ..TestWindowParams::new(id)
    };

    layout.add_window(
        TestWindow::new(params),
        AddWindowTarget::Auto,
        None,
        None,
        false,
        is_floating,
        ActivateWindow::default(),
    );
    layout.verify_invariants();
}

fn add_floating_window(layout: &mut Layout<TestWindow>, id: usize, rules: ResolvedWindowRules) {
    add_window(layout, id, true, rules);
}

fn add_floating_window_to_workspace(
    layout: &mut Layout<TestWindow>,
    id: usize,
    ws_id: WorkspaceId,
    rules: ResolvedWindowRules,
) {
    let params = TestWindowParams {
        is_floating: true,
        rules: Some(rules),
        ..TestWindowParams::new(id)
    };

    layout.add_window(
        TestWindow::new(params),
        AddWindowTarget::Workspace(ws_id),
        None,
        None,
        false,
        true,
        ActivateWindow::default(),
    );
    layout.verify_invariants();
}

/// Returns the final logical top-left position of a floating window.
fn floating_window_pos(layout: &Layout<TestWindow>, id: usize) -> Option<Point<f64, Logical>> {
    layout
        .monitors()
        .flat_map(|mon| mon.workspaces.iter())
        .find_map(|ws| {
            ws.floating()
                .tiles_with_offsets()
                .find(|(tile, _)| *tile.window().id() == id)
                .map(|(_, pos)| pos)
        })
}

/// Default position of a floating window on an untouched test output.
fn default_pos() -> Point<f64, Logical> {
    Point::from(((1280. - WINDOW_SIZE.0) / 2., (720. - WINDOW_SIZE.1) / 2.))
}

#[test]
fn cursor_places_window_at_pointer_plus_offset() {
    let mut layout = new_layout(1);
    update_pointer(&mut layout, 1, Point::from((100., 200.)));

    add_floating_window(&mut layout, 1, cursor_rules(30., 40.));

    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((130., 240.)))
    );
}

#[test]
fn cursor_offset_can_be_negative() {
    let mut layout = new_layout(1);
    update_pointer(&mut layout, 1, Point::from((200., 200.)));

    add_floating_window(&mut layout, 1, cursor_rules(-50., -60.));

    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((150., 140.)))
    );
}

#[test]
fn cursor_placement_is_clamped_within_working_area() {
    let mut layout = new_layout(1);
    update_pointer(&mut layout, 1, Point::from((1270., 710.)));

    add_floating_window(&mut layout, 1, cursor_rules(25., 35.));

    // Clamping prefers the top-left corner, so the window stays fully inside the output.
    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((1280. - WINDOW_SIZE.0, 720. - WINDOW_SIZE.1)))
    );
}

#[test]
fn cursor_falls_back_to_default_without_pointer_position() {
    let mut layout = new_layout(1);

    add_floating_window(&mut layout, 1, cursor_rules(30., 40.));

    assert_eq!(floating_window_pos(&layout, 1), Some(default_pos()));
}

#[test]
fn cursor_position_is_cleared_by_none() {
    let mut layout = new_layout(1);
    update_pointer(&mut layout, 1, Point::from((100., 200.)));
    clear_pointer(&mut layout);

    add_floating_window(&mut layout, 1, cursor_rules(0., 0.));

    assert_eq!(floating_window_pos(&layout, 1), Some(default_pos()));
}

#[test]
fn cursor_ignores_pointer_on_another_output() {
    let mut layout = new_layout(2);
    Op::FocusOutput(1).apply(&mut layout);
    layout.verify_invariants();

    // The pointer is over output2, but the window is added to output1.
    update_pointer(&mut layout, 2, Point::from((100., 200.)));
    add_floating_window(&mut layout, 1, cursor_rules(0., 0.));

    assert_eq!(floating_window_pos(&layout, 1), Some(default_pos()));
}

#[test]
fn cursor_ignores_pointer_on_another_workspace() {
    let mut layout = new_layout(1);

    // Adding a window to the last workspace creates an empty workspace below it.
    add_window(&mut layout, 1, false, ResolvedWindowRules::default());
    let other_ws = layout.monitors().next().unwrap().workspaces[1].id();

    // The pointer is over the first workspace, but the window goes to the second one.
    update_pointer(&mut layout, 1, Point::from((100., 200.)));
    add_floating_window_to_workspace(&mut layout, 2, other_ws, cursor_rules(0., 0.));

    assert_eq!(floating_window_pos(&layout, 2), Some(default_pos()));
}

#[test]
fn cursor_state_does_not_leak_after_workspace_switch() {
    let mut layout = new_layout(1);

    // Adding a window to the last workspace creates an empty workspace below it.
    add_window(&mut layout, 1, false, ResolvedWindowRules::default());

    // Mark the first workspace with a pointer position.
    update_pointer(&mut layout, 1, Point::from((100., 200.)));

    // Switch to the second workspace; the pointer now belongs to it only.
    Op::FocusWorkspaceDown.apply(&mut layout);
    Op::CompleteAnimations.apply(&mut layout);
    update_pointer(&mut layout, 1, Point::from((300., 400.)));

    add_floating_window(&mut layout, 2, cursor_rules(0., 0.));
    assert_eq!(
        floating_window_pos(&layout, 2),
        Some(Point::from((300., 400.)))
    );

    // Going back must not resurrect the pointer position from before the switch.
    Op::FocusWorkspaceUp.apply(&mut layout);
    Op::CompleteAnimations.apply(&mut layout);

    add_floating_window(&mut layout, 3, cursor_rules(0., 0.));
    assert_eq!(floating_window_pos(&layout, 3), Some(default_pos()));
}

#[test]
fn cursor_placement_on_scaled_output() {
    let mut layout = Layout::default();
    Op::AddScaledOutput {
        id: 1,
        scale: 2.,
        layout_config: None,
    }
    .apply(&mut layout);
    layout.verify_invariants();

    // A 1280 × 720 output with scale 2 has a 640 × 360 logical size.
    update_pointer(&mut layout, 1, Point::from((50., 60.)));

    add_floating_window(&mut layout, 1, cursor_rules(10., 20.));

    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((60., 80.)))
    );
}

#[test]
fn cursor_placement_ignores_tiling_struts() {
    let options = Options {
        layout: niri_config::Layout {
            struts: Struts {
                left: FloatOrInt(100.),
                right: FloatOrInt(100.),
                top: FloatOrInt(100.),
                bottom: FloatOrInt(100.),
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let mut layout = Layout::with_options(Clock::with_time(Duration::ZERO), options);
    Op::AddOutput(1).apply(&mut layout);
    layout.verify_invariants();

    update_pointer(&mut layout, 1, Point::from((100., 200.)));

    add_floating_window(&mut layout, 1, cursor_rules(0., 0.));

    // Struts only affect the tiling layout, so the floating position is unaffected.
    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((100., 200.)))
    );
}

#[test]
fn stored_floating_position_wins_over_cursor() {
    let mut layout = new_layout(1);
    update_pointer(&mut layout, 1, Point::from((100., 200.)));

    add_floating_window(&mut layout, 1, cursor_rules(0., 0.));
    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((100., 200.)))
    );

    // Send the window to the tiling layout and float it again. It must return to where it was,
    // not to the current pointer position.
    Op::ToggleWindowFloating { id: Some(1) }.apply(&mut layout);
    update_pointer(&mut layout, 1, Point::from((600., 500.)));
    Op::ToggleWindowFloating { id: Some(1) }.apply(&mut layout);

    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((100., 200.)))
    );
}

#[test]
fn first_tiled_to_floating_uses_cursor() {
    let mut layout = new_layout(1);
    add_window(&mut layout, 1, false, cursor_rules(0., 0.));

    // A window that becomes floating keeps its tiled size, so the placement is clamped to the
    // working area with that size. The origin is the only position that is never clamped, and the
    // tiling fallback position would be further right and down.
    update_pointer(&mut layout, 1, Point::from((0., 0.)));
    Op::ToggleWindowFloating { id: Some(1) }.apply(&mut layout);
    layout.verify_invariants();

    assert_eq!(floating_window_pos(&layout, 1), Some(Point::from((0., 0.))));
}

#[test]
fn non_cursor_relative_to_values_are_unchanged() {
    let mut layout = new_layout(1);

    add_floating_window(
        &mut layout,
        1,
        position_rules(RelativeTo::TopLeft, 10., 20.),
    );
    add_floating_window(
        &mut layout,
        2,
        position_rules(RelativeTo::BottomRight, 10., 20.),
    );
    add_floating_window(&mut layout, 3, position_rules(RelativeTo::Top, 0., 0.));

    assert_eq!(
        floating_window_pos(&layout, 1),
        Some(Point::from((10., 20.)))
    );
    assert_eq!(
        floating_window_pos(&layout, 2),
        Some(Point::from((
            1280. - WINDOW_SIZE.0 - 10.,
            720. - WINDOW_SIZE.1 - 20.
        )))
    );
    assert_eq!(
        floating_window_pos(&layout, 3),
        Some(Point::from(((1280. - WINDOW_SIZE.0) / 2., 0.)))
    );
}

/// Opens the overview with two workspaces and returns the active workspace's render geometry.
fn overview_active_workspace_geo(layout: &mut Layout<TestWindow>) -> Rectangle<f64, Logical> {
    add_window(layout, 1, false, ResolvedWindowRules::default());
    Op::FocusWorkspace(1).apply(layout);
    Op::CompleteAnimations.apply(layout);
    Op::ToggleOverview.apply(layout);
    Op::CompleteAnimations.apply(layout);
    layout.verify_invariants();

    let mon = layout.monitors().next().unwrap();
    mon.workspaces_render_geo()
        .nth(mon.active_workspace_idx())
        .unwrap()
}

#[test]
fn cursor_placement_undoes_overview_zoom() {
    let mut layout = new_layout(1);
    let geo = overview_active_workspace_geo(&mut layout);

    let mon = layout.monitors().next().unwrap();
    let zoom = mon.overview_zoom();
    assert!(
        0. < zoom && zoom < 1.,
        "the overview must be open for this test"
    );

    let pointer = geo.loc + Point::from((40., 30.));
    assert!(
        geo.contains(pointer),
        "the pointer must be over the workspace"
    );

    update_pointer(&mut layout, 1, pointer);
    add_floating_window(&mut layout, 2, cursor_rules(0., 0.));

    // The pointer position is converted from output coordinates to workspace coordinates.
    let expected = (pointer - geo.loc).downscale(zoom);
    assert_eq!(floating_window_pos(&layout, 2), Some(expected));
}

#[test]
fn cursor_falls_back_when_overview_pointer_is_on_another_workspace() {
    let mut layout = new_layout(1);
    let active_geo = overview_active_workspace_geo(&mut layout);

    let mon = layout.monitors().next().unwrap();
    let output_size = Rectangle::from_size(mon.view_size());
    let other_geo = mon
        .workspaces_render_geo()
        .find(|geo| *geo != active_geo)
        .unwrap();

    let Some(visible) = other_geo.intersection(output_size) else {
        panic!("the other workspace must be visible in the overview");
    };
    let pointer = visible.loc + visible.size.to_point().downscale(2.);

    // The pointer is over a workspace that isn't the active one, so the window that gets added to
    // the active workspace must fall back to the default position.
    update_pointer(&mut layout, 1, pointer);
    add_floating_window(&mut layout, 2, cursor_rules(0., 0.));

    assert_eq!(floating_window_pos(&layout, 2), Some(default_pos()));
}
