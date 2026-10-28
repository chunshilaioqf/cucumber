//! 构建脚本：注入版本与构建信息（需求 7.7），供「关于」页与 /api/health 展示。

fn main() {
    println!("cargo:rustc-env=APP_VERSION={}", env!("CARGO_PKG_VERSION"));
    println!("cargo:rerun-if-changed=build.rs");

    // git 短哈希：无 .git 环境（如源码包构建）时回退 unknown
    let git_sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=GIT_SHA={git_sha}");
    println!(
        "cargo:rustc-env=BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_default()
    );
}
