//! 检测本机是否有其他 VPN / 系统代理在抢流量。
//!
//! 用户电脑上常见 Shadowrocket、Clash Verge、Surge 等会：
//! 1. 建 utun 接管默认路由（Telegram 等不走 HTTP 代理的 App 全被它吃掉）
//! 2. 把 scutil 系统代理改成自己的端口（例如 1082）
//!
//! Pika 开「系统代理」时必须能发现这件事：提示用户先关掉对方，
//! 对方一关就立刻把 HTTP/HTTPS/SOCKS 抢回 12080。

use serde::Serialize;
use std::process::Command;
use tauri::Emitter;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VpnConflict {
    pub name: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VpnConflictReport {
    /// 有其他 VPN 已连上（utun 抢走默认路由）。这时改系统代理也没用。
    pub has_conflict: bool,
    /// scutil 系统代理指向了别人的端口。VPN 一断就可以抢回来。
    pub proxy_hijacked: bool,
    pub conflicts: Vec<VpnConflict>,
}

impl VpnConflictReport {
    pub fn empty() -> Self {
        Self {
            has_conflict: false,
            proxy_hijacked: false,
            conflicts: Vec::new(),
        }
    }

    pub fn summary(&self) -> String {
        if self.conflicts.is_empty() {
            return String::new();
        }
        self.conflicts
            .iter()
            .map(|c| format!("{}（{}）", c.name, c.kind))
            .collect::<Vec<_>>()
            .join("、")
    }
}

/// 扫描当前是否有其他 VPN / 系统代理在抢流量。
///
/// 注意：macOS 上 `scutil --proxy` 在有 utun 时经常显示别人的端口（例如 1082），
/// 真正给 App 用的是 `networksetup -getwebproxy Wi-Fi`。冲突检测只看 VPN 是否
/// 抢走默认路由，不再把 scutil 1082 当成「被其他软件占用」。
pub fn detect_vpn_conflicts(our_proxy_port: u16) -> VpnConflictReport {
    let _ = our_proxy_port;
    let vpn_services = detect_connected_vpn_services();
    let mut conflicts = vpn_services;
    conflicts.sort_by(|a, b| a.name.cmp(&b.name).then(a.kind.cmp(&b.kind)));
    conflicts.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

    VpnConflictReport {
        has_conflict: conflicts.iter().any(|c| c.kind == "vpn"),
        proxy_hijacked: false,
        conflicts,
    }
}

fn detect_connected_vpn_services() -> Vec<VpnConflict> {
    #[cfg(target_os = "macos")]
    {
        let mut found = Vec::new();

        // 系统设置里的开关比 scutil --nc 更准：Shadowrocket 断开后 --nc 仍可能报 Connected。
        // 真正抢走流量的标志是默认路由还在 utun 上。
        let default_iface = default_route_interface();
        let default_is_utun = default_iface
            .as_deref()
            .map(|i| i.starts_with("utun"))
            .unwrap_or(false);

        // 只看默认路由。scutil --nc 在系统设置断开后仍可能报 Connected，不能当事实源。
        if default_is_utun {
            let name = default_iface
                .as_deref()
                .and_then(|iface| vpn_name_for_utun(iface))
                .unwrap_or_else(|| "系统 VPN".to_string());
            found.push(VpnConflict {
                name,
                kind: "vpn".to_string(),
                detail: format!(
                    "默认路由在 {}，Telegram 等 App 会走这条 VPN 而不是系统代理",
                    default_iface.clone().unwrap_or_else(|| "utun".into())
                ),
            });
        }
        return found;
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
fn default_route_interface() -> Option<String> {
    let output = Command::new("route")
        .args(["-n", "get", "default"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("interface:") {
            let name = rest.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn vpn_name_for_utun(_iface: &str) -> Option<String> {
    let output = Command::new("scutil").args(["--nc", "list"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("sing-box") || lower.contains("pika") {
            continue;
        }
        if lower.contains("(connected)") {
            return extract_quoted_name(line);
        }
    }
    None
}

fn extract_quoted_name(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    let name = rest[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// 系统代理开着时：始终把 Wi-Fi/活动网卡写回我们的端口。
/// 有其他 VPN 只提示，不挡住写入——否则用户一断开，黄条还在、代理也没接管。
pub fn reclaim_or_warn_system_proxy(
    app_handle: &tauri::AppHandle,
    our_proxy_port: u16,
    last_had_conflict: &mut bool,
) {
    use crate::utils::proxy_util::{enable_system_proxy, DEFAULT_BYPASS_LIST};

    let report = detect_vpn_conflicts(our_proxy_port);
    if report.has_conflict {
        if !*last_had_conflict {
            let summary = report.summary();
            warn!("检测到其他 VPN 占用系统：{}（仍写入本机系统代理）", summary);
            emit_vpn_conflict(app_handle, &summary);
        }
        *last_had_conflict = true;
    } else if *last_had_conflict {
        let _ = app_handle.emit(
            "vpn-conflict-cleared",
            serde_json::json!({
                "proxy_port": our_proxy_port,
                "message": "其他加速器已关闭，Pika 已接管系统代理"
            }),
        );
        *last_had_conflict = false;
    }

    if let Err(err) = enable_system_proxy("127.0.0.1", our_proxy_port, Some(DEFAULT_BYPASS_LIST)) {
        warn!("写入系统代理失败: {}", err);
    } else {
        info!("系统代理已写入 127.0.0.1:{}", our_proxy_port);
    }
    crate::app::system::compat_proxy::start_compat_proxy(our_proxy_port);
}

pub fn emit_vpn_conflict(app_handle: &tauri::AppHandle, summary: &str) {
    let _ = app_handle.emit(
        "vpn-conflict-detected",
        serde_json::json!({
            "code": "VPN_CONFLICT_DETECTED",
            "message": "本机还有其他 VPN 抢走了默认路由。浏览器可走 Pika 系统代理；Telegram 需要先断开那个 VPN。",
            "details": summary,
        }),
    );
}

#[tauri::command]
pub fn detect_vpn_conflicts_cmd(proxy_port: Option<u16>) -> VpnConflictReport {
    detect_vpn_conflicts(proxy_port.unwrap_or(12080))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_quoted_name_from_scutil_line() {
        let line = r#"* (Connected)      B1C946CD-A531-48DE-9273-8EECBF82080E VPN (com.liguangming.Shadowrocket) "Shadowrocket"                   [VPN:com.liguangming.Shadowrocket]"#;
        assert_eq!(extract_quoted_name(line).as_deref(), Some("Shadowrocket"));
    }
}
