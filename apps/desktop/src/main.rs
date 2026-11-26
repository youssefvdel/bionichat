//! bionichat Overview, styled from OpenCode Desktop's own theme tokens:
//! grey-1100 ground, layer-01 panels, alpha hairlines, 13 px UI text,
//! radius 10 panels / 6 rows, mono for commands. Fake data for now.
#![forbid(unsafe_code)]

use rune::{canvas, col, color, el, row, text, AppSpec, Area, Color, Cx, El, FillRule, Path, Shape, TextStyle, Tx};

fn c(hex: &str) -> Color {
    color::hex(hex).unwrap_or(Color::BLACK)
}

// OpenCode theme: assets/theme.json (palette grey + alpha-light), numbers.
const BG_BASE: &str = "#161616"; // grey-1100
const BG_DEEP: &str = "#080808"; // grey-1200
const LAYER_01: &str = "#242424"; // grey-1000
const LAYER_02: &str = "#2e2e2e"; // grey-900
const TEXT_BASE: &str = "#fafafa"; // grey-100
const TEXT_MUTED: &str = "#aeaeae"; // grey-500
const TEXT_FAINT: &str = "#808080"; // grey-600
const ACCENT: &str = "#7698fd"; // blue-500
const OK: &str = "#6bd586"; // green-500
const DANGER: &str = "#f17471"; // red-500

const HAIRLINE: &str = "#ffffff1a"; // alpha-light-10
const HAIRLINE_MUTED: &str = "#ffffff14"; // alpha-light-8
const OVERLAY_HOVER: &str = "#ffffff0f"; // alpha-light-6

const R_PANEL: f32 = 10.0;
const R_ROW: f32 = 6.0;

// OpenCode type: style.rs UI 13/440 lh 16 tracking -0.04, CODE 13/440 lh 20.
fn ui() -> TextStyle {
    TextStyle::new(13.0, 440.0).lh(16.0).tracking(-0.04)
}
fn ui_med() -> TextStyle {
    TextStyle::new(13.0, 530.0).lh(16.0).tracking(-0.04)
}
fn ui_small() -> TextStyle {
    TextStyle::new(12.0, 440.0).lh(16.0)
}
fn code() -> TextStyle {
    TextStyle::new(13.0, 440.0).mono().lh(20.0).tracking(-0.04)
}

/// The bionichat mark: four white strokes on a dark rounded square, drawn from
/// assets/logo/bionichat-master.svg (paths are in a 256 unit box).
const LOGO: [&str; 4] = [
    "M 133 45 L 65 45 L 45 65 L 45 133 L 51.5 139.5 L 72 119 L 72 75 L 75 72 L 119 72 L 139.5 51.5 Z",
    "M 197.5 45 L 160 45 L 45 160 L 45 197.5 Z",
    "M 58.5 211 L 96 211 L 211 96 L 211 58.5 Z",
    "M 123 211 L 191 211 L 211 191 L 211 123 L 204.5 116.5 L 184 137 L 184 181 L 181 184 L 137 184 L 116.5 204.5 Z",
];

fn logo(size: f32) -> El {
    let k = size / 256.0;
    let mut mark = el()
        .size(size, size)
        .radius(size * 0.22)
        .fill(c("#171719"))
        .border_sides([1.0, 1.0, 1.0, 1.0], c("#2d2d32"))
        .clip_rounded(size * 0.22);
    for d in LOGO {
        let area = Area::new(&Path::from_svg(d), FillRule::NonZero);
        let shape = Shape::new().fill_area(&area, Color::WHITE);
        mark = mark.child(canvas(shape).size(256.0, 256.0).origin(0.0, 0.0).scale(k).absolute(0.0, 0.0));
    }
    mark
}

fn window_button(glyph: &str, action: fn(&mut Tx)) -> El {
    row()
        .size(28.0, 24.0)
        .radius(R_ROW)
        .no_window_drag()
        .role("button")
        .label(glyph)
        .focusable(true)
        .items_center()
        .child(text(glyph).font(ui()).color(c(TEXT_MUTED)))
        .on_press_tx(move |_: &mut State, tx: &mut Tx| action(tx))
}

fn titlebar(s: &State) -> El {
    row()
        .w_full()
        .h(36.0)
        .padding(10.0)
        .gap(8.0)
        .items_center()
        .fill(c(BG_BASE))
        .border_sides([0.0, 0.0, 1.0, 0.0], c(HAIRLINE))
        .window_drag()
        .child(
            text(match s.view {
                View::Overview => "Overview",
                View::Projects => "Projects",
                View::Settings => "Settings",
            })
            .font(ui())
            .color(c(TEXT_FAINT)),
        )
        .child(el().grow(1.0))
        .child(window_button("−", |tx| tx.minimize()))
        .child(window_button("□", |tx| tx.toggle_maximized()))
        .child(window_button("✕", |tx| tx.close_window()))
}

fn dot(color: Color) -> El {
    el().size(6.0, 6.0).radius(3.0).fill(color)
}

fn sidebar(s: &State) -> El {
    col()
        .w(240.0)
        .h_full()
        .padding(12.0)
        .gap(2.0)
        .fill(c(BG_BASE))
        .border_sides([0.0, 1.0, 0.0, 0.0], c(HAIRLINE))
        .child(
            row()
                .h(36.0)
                .padding(8.0)
                .gap(8.0)
                .items_center()
                .child(logo(22.0))
                .child(text("bionichat").font(ui_med()).color(c(TEXT_BASE))),
        )
        .child(el().h(12.0))
        .child(nav("Overview", View::Overview, s))
        .child(nav("Projects", View::Projects, s))
        .child(el().grow(1.0))
        .child(nav("Settings", View::Settings, s))
}

fn nav(name: &str, view: View, s: &State) -> El {
    let active = s.view == view;
    row()
        .h(32.0)
        .padding(8.0)
        .radius(R_ROW)
        .fill(if active { c(OVERLAY_HOVER) } else { Color::TRANSPARENT })
        .role("button")
        .label(name)
        .focusable(true)
        .child(
            text(name)
                .font(if active { ui_med() } else { ui() })
                .color(if active { c(TEXT_BASE) } else { c(TEXT_MUTED) }),
        )
        .on_press(move |s: &mut State| s.view = view)
}

fn strip(s: &State) -> El {
    row()
        .h(44.0)
        .w_full()
        .padding(16.0)
        .gap(20.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], c(HAIRLINE))
        .items_center()
        .child(
            row()
                .gap(8.0)
                .items_center()
                .child(dot(if s.tunnel { c(OK) } else { c(TEXT_FAINT) }))
                .child(
                    text(if s.tunnel {
                        "kfgepc72im6k.opentunnel.xyz"
                    } else {
                        "link off"
                    })
                    .font(ui())
                    .color(if s.tunnel { c(TEXT_MUTED) } else { c(TEXT_FAINT) }),
                ),
        )
        .child(text(if s.running { "1 running" } else { "idle" }).font(ui()).color(c(TEXT_FAINT)))
        .child(el().grow(1.0))
        .child(text("Revoke").font(ui()).color(c(TEXT_FAINT)))
}

fn section(title: &str) -> El {
    text(title).font(ui_small()).color(c(TEXT_FAINT))
}

fn call_card(running: bool, tool: &str, cmd: &str, out: &str, dur: &str) -> El {
    let state_color = if running { c(ACCENT) } else { c(OK) };
    col()
        .w_full()
        .gap(8.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .items_center()
                .child(dot(state_color))
                .child(text(tool).font(ui_med()).color(c(TEXT_BASE)))
                .child(text(if running { "running" } else { "done" }).font(ui_small()).color(c(TEXT_FAINT)))
                .child(el().grow(1.0))
                .child(text(dur).font(ui_small()).color(c(TEXT_FAINT))),
        )
        .child(
            col()
                .w_full()
                .padding(10.0)
                .gap(4.0)
                .radius(R_ROW)
                .fill(c(BG_DEEP))
                .child(text(cmd).font(code()).color(c(TEXT_BASE)))
                .child(text(out).font(code()).color(if running { c(TEXT_FAINT) } else { c(TEXT_MUTED) })),
        )
}

fn board(s: &State) -> El {
    let mut inner = col().w_full().gap(10.0).child(section("RUNNING"));
    if s.running {
        inner = inner.child(call_card(true, "bash", "$ sleep 4 && echo lit-up", "…", "1.2s"));
    } else {
        inner = inner.child(
            row()
                .w_full()
                .padding(12.0)
                .radius(R_ROW)
                .fill(c(LAYER_01))
                .child(text("No calls running.").font(ui()).color(c(TEXT_MUTED))),
        );
    }
    inner = inner
        .child(el().h(14.0))
        .child(section("RECENT"))
        .child(call_card(false, "read", "README.md", "1: # bionichat", "4ms"))
        .child(call_card(false, "bash", "$ ls -la ~/Projects", "total 12  drwxr-xr-x 6 youssef staff 192", "84ms"))
        .child(call_card(false, "edit", "README.md", "replaced 1 occurrence", "12ms"));

    // Centred like Desktop's 744 px session column.
    row()
        .grow(1.0)
        .h_full()
        .padding(24.0)
        .child(el().grow(1.0))
        .child(inner.max_w(744.0).grow(1.0))
        .child(el().grow(1.0))
}

fn servers() -> El {
    col()
        .w(280.0)
        .h_full()
        .padding(16.0)
        .gap(0.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], c(HAIRLINE))
        .child(section("SERVERS"))
        .child(el().h(8.0))
        .child(server_row("core", "5 tools · 1 running", c(ACCENT), 1.0))
        .child(server_row("context7", "2 tools · idle", c(OK), 0.0))
}

fn server_row(name: &str, sub: &str, color: Color, border_top: f32) -> El {
    let r = col()
        .w_full()
        .padding(10.0)
        .gap(2.0)
        .border_sides([border_top, 0.0, 0.0, 0.0], c(HAIRLINE_MUTED))
        .child(
            row()
                .gap(8.0)
                .items_center()
                .child(dot(color))
                .child(text(name).font(ui_med()).color(c(TEXT_BASE))),
        )
        .child(text(sub).font(ui_small()).color(c(TEXT_FAINT)));
    r
}

fn page(title: &str, rows: Vec<El>) -> El {
    let mut inner = col().w_full().gap(10.0).child(section(title));
    for row in rows {
        inner = inner.child(row);
    }
    row()
        .grow(1.0)
        .h_full()
        .padding(24.0)
        .child(el().grow(1.0))
        .child(inner.max_w(744.0).grow(1.0))
        .child(el().grow(1.0))
}

fn list_row(title: &str, sub: &str) -> El {
    col()
        .w_full()
        .padding(12.0)
        .gap(2.0)
        .fill(c(LAYER_01))
        .radius(R_ROW)
        .child(text(title).font(ui_med()).color(c(TEXT_BASE)))
        .child(text(sub).font(ui_small()).color(c(TEXT_FAINT)))
}

fn projects() -> El {
    page(
        "PROJECTS",
        vec![
            list_row("Bionichat", "/home/youssefvdel/Projects/Bionichat"),
            list_row("point-here", "/home/youssefvdel/Projects/point-here"),
            list_row("Add a folder…", "opens the folder browser"),
        ],
    )
}

fn settings(s: &State) -> El {
    page(
        "LINK",
        vec![
            list_row(
                if s.tunnel { "kfgepc72im6k.opentunnel.xyz/mcp" } else { "off" },
                "anyone with this URL can run commands on this machine",
            ),
            list_row("Start with the app", "off"),
            list_row("Open with no window", "off"),
            section("FILES"),
            list_row("app data", "~/.local/share/com.bionichat.app"),
            list_row("jobs", "~/.cache/bionichat/jobs"),
        ],
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum View {
    #[default]
    Overview,
    Projects,
    Settings,
}

#[derive(Default)]
struct State {
    view: View,
    tunnel: bool,
    running: bool,
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    col()
        .w_full()
        .h_full()
        .fill(c(BG_BASE))
        .child(titlebar(s))
        .child(
            row()
                .grow(1.0)
                .w_full()
                .child(sidebar(s))
                .child(
                    col()
                        .grow(1.0)
                        .h_full()
                        .child(strip(s))
                        .child(match s.view {
                            View::Overview => row().grow(1.0).h_full().child(board(s)).child(servers()),
                            View::Projects => projects(),
                            View::Settings => settings(s),
                        }),
                ),
        )
}

fn main() {
    rune::run(AppSpec::<State> {
        scenes: &[
            ("busy", |s| {
                s.tunnel = true;
                s.running = true;
            }),
            ("idle", |s| s.tunnel = true),
            ("projects", |s| s.view = View::Projects),
            ("settings", |s| {
                s.tunnel = true;
                s.view = View::Settings;
            }),
        ],
        ..AppSpec::new("bionichat", (1280.0, 720.0), State::default, view)
    });
}
