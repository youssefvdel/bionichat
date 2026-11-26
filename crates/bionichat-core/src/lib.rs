//! bionichat's backend: the MCP hub, the tool catalog, policy, the server
//! registry and the public-link helper. No windowing, no Tauri: any shell can
//! drive it, and events go to whatever sink the shell installs.

pub mod hub;
pub mod mcp;
pub mod policy;
pub mod registry;
pub mod tunnel;

pub use hub::{EventSink, Hub};
pub use mcp::ToolInfo;
pub use registry::{Decision, Registry, ServerSpec};
pub use tunnel::{TunnelInfo, TunnelManager};

/// Child processes on Windows must not flash a console window.
pub fn hide_window(command: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}
