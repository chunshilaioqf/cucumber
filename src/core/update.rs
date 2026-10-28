//! 应用更新检查（任务 7）：启动 1 分钟后查询 GitHub Releases latest，
//! 有新版本时提醒用户：立即更新（打开下载页）/ 暂不 / 跳过此版本。
//! 无第三方更新框架：一次 API 请求 + 语义化版本比较（ADR-02 轻量原则）。

use serde_json::Value;

pub const REPO_URL: &str = "https://github.com/chunshilaioqf/cucumber";
const LATEST_API: &str = "/repos/chunshilaioqf/cucumber/releases/latest";

/// 检查结果。
pub enum UpdateCheck {
    /// 当前已是最新
    UpToDate,
    /// 有新版本（版本号、Release 页地址）
    Available { version: String, url: String },
}

/// 解析 x.y.z（忽略预发布后缀）。
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next().unwrap_or("0").parse().ok()?;
    Some((a, b, c))
}

/// 查询 GitHub API（api.github.com:443 走 TLS——http_lite 仅 http，
/// 因此这里用系统 curl 以零依赖完成 HTTPS 请求；无 curl 环境返回 UpToDate 容错）。
pub async fn check_latest() -> UpdateCheck {
    let cur = parse_semver(env!("APP_VERSION")).unwrap_or((0, 0, 0));
    let body = match tokio::process::Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "15",
            "-H",
            "Accept: application/vnd.github+json",
            &format!("https://api.github.com{LATEST_API}"),
        ])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return UpdateCheck::UpToDate,
    };
    let Ok(v) = serde_json::from_str::<Value>(&body) else {
        return UpdateCheck::UpToDate;
    };
    let tag = v.get("tag_name").and_then(Value::as_str).unwrap_or("");
    let Some(remote) = parse_semver(tag) else {
        return UpdateCheck::UpToDate;
    };
    if remote > cur {
        let url = v
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or(REPO_URL)
            .to_string();
        UpdateCheck::Available {
            version: tag.to_string(),
            url,
        }
    } else {
        UpdateCheck::UpToDate
    }
}

/// 是否应提示该版本（未被用户跳过）。
pub fn should_prompt(remote: &str, skipped: Option<&str>) -> bool {
    skipped != Some(remote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare() {
        assert!(parse_semver("v0.3.0").unwrap() > parse_semver("0.2.9").unwrap());
        assert_eq!(parse_semver("1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(parse_semver("bad"), None);
        assert!(parse_semver("10.0.0").unwrap() > parse_semver("9.9.9").unwrap());
    }

    #[test]
    fn skip_logic() {
        assert!(should_prompt("v0.3.0", None));
        assert!(should_prompt("v0.3.0", Some("v0.2.9")));
        assert!(!should_prompt("v0.3.0", Some("v0.3.0")));
    }
}
