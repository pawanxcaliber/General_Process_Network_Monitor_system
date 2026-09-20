//! Helper for spawning child processes without a console flash on Windows.

/// Apply `CREATE_NO_WINDOW` on Windows so auxiliary tools (`ping`, `netstat`,
/// `powershell`, `nvidia-smi`, ...) don't pop up console windows. The host
/// GUI process has no console to inherit, so without this flag each spawn
/// allocates a fresh one.
#[allow(unused)]
pub fn set_no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

#[allow(unused)]
pub fn set_no_window_t(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}
