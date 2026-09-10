use std::io;

#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use tracing::info;

#[cfg(target_os = "windows")]
use crate::app::constants::registry;
#[cfg(target_os = "windows")]
use tracing::warn;
#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

/// 默认的系统代理绕过列表
pub const DEFAULT_BYPASS_LIST: &str =
    "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;\
172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*";

fn parse_bypass_entries(raw: Option<&str>) -> Vec<String> {
    let source = raw
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_BYPASS_LIST);

    source
        .split([';', ',', '\n'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// 禁用系统代理 (跨平台实现)
pub fn disable_system_proxy() -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        disable_system_proxy_windows()
    }

    #[cfg(target_os = "linux")]
    {
        disable_system_proxy_linux()
    }

    #[cfg(target_os = "macos")]
    {
        disable_system_proxy_macos()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Ok(()) // 其他平台暂时不执行任何操作
    }
}

/// 启用系统代理 (跨平台实现)
pub fn enable_system_proxy(host: &str, port: u16, bypass: Option<&str>) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        enable_system_proxy_windows(host, port, bypass)
    }

    #[cfg(target_os = "linux")]
    {
        enable_system_proxy_linux(host, port, bypass)
    }

    #[cfg(target_os = "macos")]
    {
        enable_system_proxy_macos(host, port, bypass)
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Ok(()) // 其他平台暂时不执行任何操作
    }
}

#[cfg(target_os = "windows")]
fn disable_system_proxy_windows() -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings = hkcu.open_subkey_with_flags(registry::INTERNET_SETTINGS, KEY_WRITE)?;

    // 禁用代理
    settings.set_value(registry::PROXY_ENABLE, &0u32)?;

    // 清空代理服务器地址
    settings.set_value(registry::PROXY_SERVER, &"")?;

    // 通知 WinINet 重新读取设置，使基于 WinINet 的应用（Edge/Chrome/IE 等）立即生效。
    notify_wininet_change();

    Ok(())
}

#[cfg(target_os = "linux")]
fn disable_system_proxy_linux() -> io::Result<()> {
    // Linux下的系统代理设置通常通过环境变量
    // 这里可以尝试使用gsettings或者直接设置环境变量
    std::env::remove_var("http_proxy");
    std::env::remove_var("https_proxy");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("all_proxy");
    std::env::remove_var("ALL_PROXY");
    std::env::remove_var("no_proxy");
    std::env::remove_var("NO_PROXY");

    // 尝试使用gsettings重置代理设置 (GNOME/Unity/XFCE等)
    if std::process::Command::new("which")
        .arg("gsettings")
        .output()
        .is_ok()
    {
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.http", "host", "''"])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.http", "port", "0"])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.https", "host", "''"])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.https", "port", "0"])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "'none'"])
            .output();
    }

    // 尝试使用kwriteconfig5/6重置代理设置 (KDE Plasma)
    for kwriteconfig in &["kwriteconfig6", "kwriteconfig5"] {
        if std::process::Command::new("which")
            .arg(kwriteconfig)
            .output()
            .is_ok()
        {
            // 设置代理模式为无代理 (0)
            let _ = std::process::Command::new(kwriteconfig)
                .args([
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    "ProxyType",
                    "0",
                ])
                .output();

            // 通知KDE配置已更改
            let _ = std::process::Command::new("dbus-send")
                .args([
                    "--type=signal",
                    "/KIO/Scheduler",
                    "org.kde.KIO.Scheduler.reparseSlaveConfiguration",
                    "string:''",
                ])
                .output();
            break;
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_network_services() -> Vec<String> {
    let output = Command::new("networksetup")
        .args(["-listallnetworkservices"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "*")
        .map(|s| s.trim_start_matches('*').trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 对照同事 PR #7（KTAIorg/pika `macos_proxy.rs`）：
/// 只打用户会用的网卡（Wi-Fi / Ethernet / USB LAN），跳过 Shadowrocket 和假 USB 口。
/// 这台机器 Ethernet(en0) 是空的，真正上网的是 Wi-Fi(en1)；不能按列表顺序取第一个。
#[cfg(target_os = "macos")]
fn is_user_facing_service(name: &str) -> bool {
    matches!(name, "Wi-Fi" | "Ethernet" | "USB 10/100/1000 LAN")
        || name.starts_with("Wi-Fi")
        || name.eq_ignore_ascii_case("wi-fi")
}

fn is_vpn_service_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("shadowrocket")
        || lower.contains("clash")
        || lower.contains("surge")
        || lower.contains("stash")
        || lower.contains("wireguard")
        || lower.contains("tailscale")
        || lower == "vpn"
        || lower.contains(" vpn")
}

#[cfg(target_os = "macos")]
fn parse_service_device(order_text: &str, service: &str) -> Option<String> {
    // 格式：(17) Wi-Fi
    //       (Hardware Port: Wi-Fi, Device: en1)
    let mut current: Option<&str> = None;
    for line in order_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('(') {
            if rest.starts_with(char::is_numeric) {
                let name = rest
                    .find(')')
                    .map(|i| rest[i + 1..].trim())
                    .unwrap_or("")
                    .trim_start_matches('*')
                    .trim();
                current = if name.is_empty() { None } else { Some(name) };
                continue;
            }
        }
        if current == Some(service) {
            if let Some(idx) = trimmed.find("Device:") {
                let device = trimmed[idx + "Device:".len()..]
                    .trim()
                    .trim_end_matches(')')
                    .trim();
                if !device.is_empty() {
                    return Some(device.to_string());
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn iface_has_ipv4(iface: &str) -> bool {
    if iface.is_empty() || iface.starts_with("utun") || iface.starts_with("lo") {
        return false;
    }
    Command::new("ipconfig")
        .args(["getifaddr", iface])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn macos_active_network_services() -> Vec<String> {
    let all = macos_network_services();
    let order = Command::new("networksetup")
        .args(["-listnetworkserviceorder"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    // 客户关小火箭后，Agent/CLI 看的是 scutil（当前 VPN 服务的代理），不是 Wi-Fi。
    // 所以除了 Wi-Fi/Ethernet，还要把「当前这张 VPN 网卡」写成我们的端口。
    let facing: Vec<String> = all
        .into_iter()
        .filter(|name| is_user_facing_service(name) || is_vpn_service_name(name))
        .collect();

    // 优先写「现在有 IPv4」的那张（这台机器是 Wi-Fi/en1，不是 Ethernet/en0）。
    let mut live: Vec<String> = facing
        .iter()
        .filter(|name| {
            parse_service_device(&order, name)
                .map(|dev| iface_has_ipv4(&dev))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    // Wi-Fi 始终写入：系统设置里用户看到的就是它；同事 PR 也是 Wi-Fi + Ethernet 一起打。
    for name in &facing {
        if name.eq_ignore_ascii_case("wi-fi") || name.starts_with("Wi-Fi") {
            if !live.iter().any(|s| s == name) {
                live.insert(0, name.clone());
            }
        }
    }

    if live.is_empty() {
        live = facing;
    }
    live.sort();
    live.dedup();
    info!("macOS 系统代理将写入网卡: {:?}", live);
    live
}

#[cfg(target_os = "macos")]
fn disable_system_proxy_macos() -> io::Result<()> {
    for service in macos_active_network_services() {
        let _ = Command::new("networksetup")
            .args(["-setwebproxystate", &service, "off"])
            .output();
        let _ = Command::new("networksetup")
            .args(["-setsecurewebproxystate", &service, "off"])
            .output();
        let _ = Command::new("networksetup")
            .args(["-setsocksfirewallproxystate", &service, "off"])
            .output();
    }

    // 同时清除环境变量
    std::env::remove_var("http_proxy");
    std::env::remove_var("https_proxy");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("all_proxy");
    std::env::remove_var("ALL_PROXY");
    clear_user_proxy_env();

    Ok(())
}

#[cfg(target_os = "macos")]
#[cfg(test)]
mod macos_proxy_tests {
    use super::{is_user_facing_service, parse_service_device};

    #[test]
    fn user_facing_matches_colleague_pr7() {
        assert!(is_user_facing_service("Wi-Fi"));
        assert!(is_user_facing_service("Ethernet"));
        assert!(is_user_facing_service("USB 10/100/1000 LAN"));
        assert!(!is_user_facing_service("Thunderbolt Bridge"));
        assert!(!is_user_facing_service("Shadowrocket"));
        assert!(!is_user_facing_service("USB 10/100/1000 LAN 2"));
    }

    #[test]
    fn parse_wifi_device_from_service_order() {
        let order = "\
(1) Ethernet\n\
(Hardware Port: Ethernet, Device: en0)\n\
\n\
(17) Wi-Fi\n\
(Hardware Port: Wi-Fi, Device: en1)\n";
        assert_eq!(parse_service_device(order, "Ethernet").as_deref(), Some("en0"));
        assert_eq!(parse_service_device(order, "Wi-Fi").as_deref(), Some("en1"));
    }
}

#[cfg(target_os = "windows")]
fn enable_system_proxy_windows(host: &str, port: u16, bypass: Option<&str>) -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings = hkcu.open_subkey_with_flags(registry::INTERNET_SETTINGS, KEY_WRITE)?;

    // 设置代理服务器地址
    let proxy_server = format!("{}:{}", host, port);
    settings.set_value(registry::PROXY_SERVER, &proxy_server)?;

    // 启用代理
    settings.set_value(registry::PROXY_ENABLE, &1u32)?;

    // 设置绕过本地地址
    let entries = parse_bypass_entries(bypass);
    let override_value = if entries.is_empty() {
        DEFAULT_BYPASS_LIST.to_string()
    } else {
        entries.join(";")
    };
    settings.set_value(registry::PROXY_OVERRIDE, &override_value)?;

    // 通知 WinINet 重新读取设置，使基于 WinINet 的应用（Edge/Chrome/IE 等）立即生效。
    notify_wininet_change();

    Ok(())
}

/// 通知 WinINet 配置已变更并刷新，使系统代理设置即时生效。
///
/// 仅写注册表而不调用本函数时，基于 WinINet 的应用（Edge/Chrome/IE/资源管理器等）
/// 不会主动重新读取代理设置，表现为“代理已写入但浏览器仍连不上”，需要手动重启才能恢复。
///
/// 这里调用两个选项：`INTERNET_OPTION_SETTINGS_CHANGED`（通知设置变更）+
/// `INTERNET_OPTION_REFRESH`（强制重新读取）。任一失败仅记录警告，不阻断代理写入。
///
/// 实现说明：直接用 FFI 声明 + `#[link(name = "wininet")]` 链接系统库，
/// 避免引入 `windows` crate 的庞大 Win32 feature（会拖慢甚至拖崩编译器）。
#[cfg(target_os = "windows")]
fn notify_wininet_change() {
    // Win32 常量（来自 wininet.h）。
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
    const INTERNET_OPTION_REFRESH: u32 = 37;

    #[link(name = "wininet")]
    extern "system" {
        fn InternetSetOptionW(
            hinternet: *mut std::ffi::c_void,
            option: u32,
            buffer: *mut std::ffi::c_void,
            buffer_length: u32,
        ) -> i32;
    }

    // SAFETY: INTERNET_OPTION_SETTINGS_CHANGED / REFRESH 不需要 buffer
    // （lpBuffer=NULL，dwBufferLength=0），是 Win32 API 文档允许的用法；
    // hInternet 对这两个选项也必须为 NULL。函数无不可控副作用。
    unsafe {
        let hinternet = std::ptr::null_mut::<std::ffi::c_void>();
        let buffer = std::ptr::null_mut::<std::ffi::c_void>();

        if InternetSetOptionW(hinternet, INTERNET_OPTION_SETTINGS_CHANGED, buffer, 0) == 0 {
            warn!("InternetSetOptionW(SETTINGS_CHANGED) 返回 0");
        }

        if InternetSetOptionW(hinternet, INTERNET_OPTION_REFRESH, buffer, 0) == 0 {
            warn!("InternetSetOptionW(REFRESH) 返回 0");
        }
    }
}

#[cfg(target_os = "linux")]
fn enable_system_proxy_linux(host: &str, port: u16, bypass: Option<&str>) -> io::Result<()> {
    let proxy_url = format!("http://{}:{}", host, port);
    let proxy_url_secure = format!("https://{}:{}", host, port);
    let entries = parse_bypass_entries(bypass);
    let no_proxy = if entries.is_empty() {
        DEFAULT_BYPASS_LIST.replace(';', ",")
    } else {
        entries.join(",")
    };

    // 设置环境变量
    std::env::set_var("http_proxy", &proxy_url);
    std::env::set_var("https_proxy", &proxy_url_secure);
    std::env::set_var("HTTP_PROXY", &proxy_url);
    std::env::set_var("HTTPS_PROXY", &proxy_url_secure);
    std::env::set_var("all_proxy", &proxy_url);
    std::env::set_var("ALL_PROXY", &proxy_url);
    std::env::set_var("no_proxy", &no_proxy);
    std::env::set_var("NO_PROXY", &no_proxy);

    // 尝试使用gsettings设置代理 (GNOME/Unity/XFCE等)
    if std::process::Command::new("which")
        .arg("gsettings")
        .output()
        .is_ok()
    {
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.http", "host", host])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.system.proxy.http",
                "port",
                &port.to_string(),
            ])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.https", "host", host])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.system.proxy.https",
                "port",
                &port.to_string(),
            ])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "'manual'"])
            .output();
    }

    // 尝试使用kwriteconfig5/6设置代理 (KDE Plasma)
    for kwriteconfig in &["kwriteconfig6", "kwriteconfig5"] {
        if std::process::Command::new("which")
            .arg(kwriteconfig)
            .output()
            .is_ok()
        {
            let proxy_url = format!("http://{}:{}", host, port);

            // 设置HTTP代理
            let _ = std::process::Command::new(kwriteconfig)
                .args([
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    "httpProxy",
                    &proxy_url,
                ])
                .output();

            // 设置HTTPS代理
            let _ = std::process::Command::new(kwriteconfig)
                .args([
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    "httpsProxy",
                    &proxy_url,
                ])
                .output();

            // 设置代理模式为手动 (1)
            let _ = std::process::Command::new(kwriteconfig)
                .args([
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    "ProxyType",
                    "1",
                ])
                .output();

            // 通知KDE配置已更改
            let _ = std::process::Command::new("dbus-send")
                .args([
                    "--type=signal",
                    "/KIO/Scheduler",
                    "org.kde.KIO.Scheduler.reparseSlaveConfiguration",
                    "string:''",
                ])
                .output();
            break;
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn enable_system_proxy_macos(host: &str, port: u16, bypass: Option<&str>) -> io::Result<()> {
    let entries = parse_bypass_entries(bypass);
    let port_s = port.to_string();

    for service in macos_active_network_services() {
        let _ = Command::new("networksetup")
            .args(["-setwebproxy", &service, host, &port_s])
            .output();
        let _ = Command::new("networksetup")
            .args(["-setwebproxystate", &service, "on"])
            .output();
        let _ = Command::new("networksetup")
            .args(["-setsecurewebproxy", &service, host, &port_s])
            .output();
        let _ = Command::new("networksetup")
            .args(["-setsecurewebproxystate", &service, "on"])
            .output();
        // mixed inbound 同时提供 HTTP 和 SOCKS5。Telegram 等不走 HTTP 代理的 App 需要 SOCKS。
        let _ = Command::new("networksetup")
            .args(["-setsocksfirewallproxy", &service, host, &port_s])
            .output();
        let _ = Command::new("networksetup")
            .args(["-setsocksfirewallproxystate", &service, "on"])
            .output();

        if !entries.is_empty() {
            let mut cmd = Command::new("networksetup");
            cmd.args(["-setproxybypassdomains", &service]);
            for entry in &entries {
                cmd.arg(entry);
            }
            let _ = cmd.output();
        }
    }

    // 同时设置环境变量
    let proxy_url = format!("http://{}:{}", host, port);
    let proxy_url_secure = format!("https://{}:{}", host, port);

    std::env::set_var("http_proxy", &proxy_url);
    std::env::set_var("https_proxy", &proxy_url_secure);
    std::env::set_var("HTTP_PROXY", &proxy_url);
    std::env::set_var("HTTPS_PROXY", &proxy_url_secure);
    std::env::set_var("all_proxy", &proxy_url);
    std::env::set_var("ALL_PROXY", &proxy_url);
    publish_user_proxy_env(host, port);

    Ok(())
}

/// 给当前用户写 launchctl 环境，让新拉起的 Agent/终端也走我们的端口，
/// 而不是机器里残留的 HTTP_PROXY=1082。
#[cfg(target_os = "macos")]
fn publish_user_proxy_env(host: &str, port: u16) {
    let http = format!("http://{}:{}", host, port);
    let socks = format!("socks5h://{}:{}", host, port);
    let no_proxy = "localhost,127.0.0.1,::1,.local";
    let pairs = [
        ("http_proxy", http.as_str()),
        ("https_proxy", http.as_str()),
        ("HTTP_PROXY", http.as_str()),
        ("HTTPS_PROXY", http.as_str()),
        ("ALL_PROXY", socks.as_str()),
        ("all_proxy", socks.as_str()),
        ("NO_PROXY", no_proxy),
        ("no_proxy", no_proxy),
    ];
    for (key, value) in pairs {
        let _ = Command::new("launchctl")
            .args(["setenv", key, value])
            .output();
    }
}

#[cfg(target_os = "macos")]
fn clear_user_proxy_env() {
    for key in [
        "http_proxy",
        "https_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        let _ = Command::new("launchctl").args(["unsetenv", key]).output();
    }
}
