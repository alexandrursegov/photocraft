//! Photoshop shows the active function in the pointer: held ⌘ (Ctrl off the Mac) is the move
//! cursor and ⌥ added to it the white-and-black duplicate pair (`canvas::move_pointer`), the
//! Move tool's own drag the same, and the lasso family, the Magic Wand and the Object Selection
//! tool carry their tool glyph as Photoshop's standard cursors (Other Cursors: Precise stays the
//! bare crosshair, like the pens and the bucket).

use egui::{CursorIcon, Event, Modifiers, vec2};
use egui_kittest::Harness;
use serde_json::json;

use crate::PhotocraftApp;
use crate::canvas::{Drag, ViewXform, move_pointer, move_pointer_copy};
use crate::state::Tool;

fn harness(tool: Tool) -> Harness<'static, PhotocraftApp> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app.sync_views();
    app.ui.extras.rulers = false;
    app.ui.tool = tool;
    let mut h = Harness::builder().with_size(vec2(1000.0, 700.0)).build_ui_state(
        |ui, app: &mut PhotocraftApp| {
            let ctx = ui.ctx().clone();
            if !ctx.fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                return;
            }
            crate::shortcuts::handle(app, &ctx);
            egui::CentralPanel::default().show(ui, |ui| crate::canvas::document_area(app, ui));
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
    h.run_steps(4);
    let v = &mut h.state_mut().ui.views[0];
    v.zoom = 1.0;
    v.center = [200.0, 150.0];
    v.fit_pending = false;
    h.run_steps(2);
    h
}

fn doc_screen(h: &Harness<'static, PhotocraftApp>, x: f32, y: f32) -> egui::Pos2 {
    let app = h.state();
    let v = &app.ui.views[0];
    let xf = ViewXform {
        rect: crate::rulers::content_rect(app, app.last_canvas_rect),
        zoom: v.zoom,
        center: v.center,
        flip: app.ui.view.flip_horizontal,
        rotation: v.rotation,
        aspect: app.ui.view.display_aspect(),
    };
    xf.to_screen(x, y)
}

/// The pointer over the document point with `m` held: the OS icon and whether a bitmap cursor
/// (the brush tip, the crosshair) replaced it instead of an Area-drawn tool icon.
fn pointer_over(h: &mut Harness<'static, PhotocraftApp>, x: f32, y: f32, m: Modifiers) -> (CursorIcon, Option<[u16; 2]>) {
    h.event(Event::ModifiersChanged(m));
    let p = doc_screen(h, x, y);
    h.event(Event::PointerMoved(p));
    h.run_steps(2);
    let out = h.output();
    (out.platform_output.cursor_icon, out.platform_output.cursor_image.as_ref().map(|i| i.size))
}

/// ⌘ held with the Brush hands the pointer to the move icon: the brush tip's bitmap is gone and
/// the move glyph is drawn at the point; releasing ⌘ brings the tip back.
#[test]
fn cmd_held_replaces_the_brush_tip_with_the_move_pointer() {
    let mut h = harness(Tool::Brush);
    h.state_mut().run("tools.setBrush", json!({"brush": {"size": 80}})).unwrap();
    h.run_steps(1);
    let (_, tip) = pointer_over(&mut h, 150.0, 100.0, Modifiers::NONE);
    let side = tip.expect("the brush tip is a bitmap cursor")[0];
    assert!(side > 60, "an 80 px brush tip: {side}");
    let tip_shapes = h.output().shapes.len();
    let (icon, img) = pointer_over(&mut h, 150.0, 100.0, Modifiers::COMMAND);
    assert_eq!((icon, img), (CursorIcon::None, None), "⌘: the move icon is drawn at the point itself");
    assert!(h.output().shapes.len() > tip_shapes, "the move glyph is on screen");
    let (_, back) = pointer_over(&mut h, 150.0, 100.0, Modifiers::NONE);
    assert_eq!(back.map(|s| s[0]), Some(side), "the Brush is back when ⌘ comes up");
}

/// The lasso family and Object Selection keep the crosshair and add their glyph beside it; the
/// Magic Wand is its own drawn pointer. Other Cursors: Precise takes the glyph away.
#[test]
fn selection_tools_show_their_glyph_and_precise_takes_it_away() {
    for tool in [Tool::Lasso, Tool::PolygonLasso, Tool::ObjectSelection, Tool::MagicWand] {
        let mut h = harness(tool);
        let (_, _) = pointer_over(&mut h, 150.0, 100.0, Modifiers::NONE);
        let standard = h.output().shapes.len();
        h.state_mut().session.prefs.edit(|p| p.cursors.other = photocraft_engine::prefs::OtherCursor::Precise);
        h.run_steps(4);
        let (icon, _) = pointer_over(&mut h, 150.0, 100.0, Modifiers::NONE);
        assert_eq!(icon, CursorIcon::Crosshair, "{tool:?}: Precise is the crosshair");
        assert!(standard > h.output().shapes.len(), "{tool:?}: the standard cursor draws the tool glyph too");
    }
}

/// The pure decision: the temporary Move or a Move-tool drag shows the move pointer, and the
/// duplicate pair follows ⌥ — from the drag's press modifiers once a drag runs.
#[test]
fn the_move_pointer_and_its_duplicate_are_decided_from_the_modifiers() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let cmd = Modifiers::COMMAND;
    assert!(!move_pointer(&app, None), "no ⌘, no drag: the tool's own pointer");
    assert!(move_pointer(&app, Some(crate::hold_keys::Temporary::Move)), "held ⌘ is the move pointer");
    assert!(!move_pointer_copy(&app, cmd), "⌘ alone moves");
    assert!(move_pointer_copy(&app, Modifiers { alt: true, command: true, ..Default::default() }), "⌘⌥ duplicates");

    let mut app = app;
    app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    let plain = Drag::new(Tool::Move, [10.0, 10.0], vec![[10.0, 10.0, 1.0]], Modifiers::NONE, false);
    let with_alt = Drag::new(Tool::Move, [10.0, 10.0], vec![[10.0, 10.0, 1.0]], Modifiers { alt: true, ..Default::default() }, false);
    app.drag = Some(plain);
    assert!(move_pointer(&app, None), "a Move-tool drag shows the move pointer");
    assert!(!move_pointer_copy(&app, Modifiers { alt: true, ..Default::default() }), "a drag without ⌥ on the press moves, never copies");
    app.drag = Some(with_alt);
    assert!(move_pointer_copy(&app, Modifiers::NONE), "⌥ on the press keeps the duplicate until release");
}
