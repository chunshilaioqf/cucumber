//! 子进程窗口抑制（0.4.3，ADR-18）：GUI 子系统进程在 Windows 上启动控制台程序
//! （curl / powershell / cmd / 用户 start_command 等）会为每个子进程创建新控制台窗口。
//! 所有子进程启动统一经 [`hide_window`] 加 `CREATE_NO_WINDOW`，
//! 保证「只有一个干净的 GUI 窗口」。

/// 为 std/tokio 命令加"不创建控制台窗口"标志（Windows；其他平台无操作）。
pub trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

#[cfg(windows)]
impl NoWindow for std::process::Command {
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

#[cfg(windows)]
impl NoWindow for tokio::process::Command {
    fn no_window(&mut self) -> &mut Self {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

#[cfg(not(windows))]
impl NoWindow for std::process::Command {
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

#[cfg(not(windows))]
impl NoWindow for tokio::process::Command {
    fn no_window(&mut self) -> &mut Self {
        self
    }
}
