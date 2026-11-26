//! bionichat Overview as a live board: running calls, recent calls, plain
//! server list. Fake data; proves the screen we actually want.
#![forbid(unsafe_code)]

use rune::{col, color, el, row, text, AppSpec, Color, Cx, El, TextStyle};

fn c(hex: &str) -> Color {
    color::hex(hex).unwrap_or(Color::BLACK)
}

const GROUND: &str = "#0e1014";
const PANEL: &str = "#14171c";
const RAISED: &str = "#191d23";
const BORDER: &str = "#23272f";
const TEXT: &str = "#e7e9ee";
const MUTED: &str = "#9aa1ad";
const FAINT: &str = "#8f97a5";
const ACCENT: &str = "#4c9dff";
const OK: &str = "#3ecf8e";

fn t(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
}

fn label(s: &str) -> El {
    text(s).font(t(11.0, 550.0).tracking(0.6)).color(c(FAINT))
}

fn mono(s: &str, size: f32) -> El {
    text(s).font(t(size, 400.0).mono()).color(c(MUTED))
}

fn dot(color: Color) -> El {
    el().size(6.0, 6.0).radius(3.0).fill(color)
}

fn sidebar() -> El {
    col()
        .w(224.0)
        .h_full()
        .padding(12.0)
        .gap(4.0)
        .fill(c("#111419"))
        .border(1.0, c(BORDER))
        .child(label("BIONICHAT"))
        .child(el().h(8.0))
        .child(nav("Overview", true))
        .child(nav("Projects", false))
        .child(el().grow(1.0))
        .child(nav("Settings", false))
        .child(mono("v0.1.0", 11.0))
}

fn nav(name: &str, active: bool) -> El {
    row()
        .h(32.0)
        .padding(8.0)
        .radius(8.0)
        .fill(if active { c(RAISED) } else { Color::TRANSPARENT })
        .child(text(name).font(t(13.0, if active { 600.0 } else { 400.0 })).color(if active { c(TEXT) } else { c(MUTED) }))
}

fn strip(s: &State) -> El {
    row()
        .h(40.0)
        .padding(12.0)
        .gap(24.0)
        .border(1.0, c(BORDER))
        .child(row().gap(6.0).child(dot(if s.tunnel { c(OK) } else { c(FAINT) })).child(mono(if s.tunnel { "kfgepc72im6k.opentunnel.xyz" } else { "off" }, 12.0)))
        .child(row().gap(6.0).child(mono("5", 12.0)).child(label("TOOLS")))
        .child(row().gap(6.0).child(mono(if s.running { "core_bash · running" } else { "—" }, 12.0)))
        .child(el().grow(1.0))
        .child(row().h(28.0).padding(10.0).radius(8.0).child(text("Revoke").font(t(12.0, 550.0)).color(c("#ff6b76"))))
}

fn call_card(running: bool, tool: &str, cmd: &str, out: &str, dur: &str) -> El {
    let accent = if running { c(ACCENT) } else { c(OK) };
    row()
        .w_full()
        .radius(8.0)
        .fill(c(PANEL))
        .border(1.0, c(BORDER))
        .child(el().w(3.0).fill(accent))
        .child(
            col()
                .grow(1.0)
                .padding(12.0)
                .gap(6.0)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(dot(accent))
                        .child(text(tool).font(t(11.0, 550.0).mono()).color(c(TEXT)))
                        .child(row().padding(6.0).radius(6.0).fill(c(RAISED)).child(text(if running { "link" } else { "local" }).font(t(11.0, 400.0).mono()).color(c(FAINT))))
                        .child(el().grow(1.0))
                        .child(mono(dur, 11.0)),
                )
                .child(text(cmd).font(t(12.0, 400.0).mono()).color(c(TEXT)))
                .child(text(out).font(t(12.0, 400.0).mono()).color(c(MUTED))),
        )
}

fn board(s: &State) -> El {
    let mut b = col().grow(1.0).h_full().padding(16.0).gap(8.0).fill(c(GROUND));
    b = b.child(label("RUNNING"));
    if s.running {
        b = b.child(call_card(true, "bash", "$ sleep 4 && echo lit-up", "…", "1.2s"));
    } else {
        b = b.child(row().h(56.0).padding(12.0).radius(8.0).border(1.0, c(BORDER)).child(text("No calls running. Start the link and connect a chat.").font(t(13.0, 400.0)).color(c(MUTED))));
    }
    b = b
        .child(el().h(8.0))
        .child(label("RECENT"))
        .child(call_card(false, "read", "README.md", "1: # bionichat", "4ms"))
        .child(call_card(false, "bash", "$ ls -la ~/Projects", "total 12  drwxr-xr-x 6 youssef staff 192", "84ms"))
        .child(call_card(false, "edit", "README.md", "replaced 1 occurrence", "12ms"));
    b
}

fn servers() -> El {
    col()
        .w(320.0)
        .h_full()
        .padding(12.0)
        .gap(8.0)
        .fill(c("#111419"))
        .border(1.0, c(BORDER))
        .child(label("SERVERS"))
        .child(server_row("bash", "2 tools · core_bash running", c(ACCENT)))
        .child(server_row("fs", "2 tools · last call 4ms", c(OK)))
        .child(server_row("skill", "1 tool · idle", c(FAINT)))
}

fn server_row(name: &str, sub: &str, color: Color) -> El {
    row()
        .w_full()
        .h(44.0)
        .padding(10.0)
        .gap(8.0)
        .radius(8.0)
        .fill(c(PANEL))
        .border(1.0, c(BORDER))
        .child(dot(color))
        .child(col().gap(2.0).grow(1.0).child(text(name).font(t(12.0, 550.0).mono()).color(c(TEXT))).child(text(sub).font(t(11.0, 400.0)).color(c(FAINT))))
}

#[derive(Default)]
struct State {
    tunnel: bool,
    running: bool,
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    row()
        .w_full()
        .h_full()
        .fill(c(GROUND))
        .child(sidebar())
        .child(col().grow(1.0).h_full().child(strip(s)).child(row().grow(1.0).h_full().child(board(s)).child(servers())))
}

fn main() {
    rune::run(AppSpec::<State> {
        scenes: &[("busy", |s| { s.tunnel = true; s.running = true }), ("idle", |s| s.tunnel = true)],
        ..AppSpec::new("bionichat overview", (1280.0, 720.0), State::default, view)
    });
}
