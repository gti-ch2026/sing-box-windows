use super::config_schema::DnsServerConfig;
use crate::app::constants::paths;
use crate::app::storage::state_model::AppConfig;
use serde_json::{json, Map, Value};
use std::net::IpAddr;
use url::Url;

// 代理组/出站标签（这些标签会暴露在 Clash API 里，尽量保持稳定，避免前端/用户习惯被破坏）。
pub const TAG_AUTO: &str = "自动选择";
pub const TAG_MANUAL: &str = "手动切换";
pub const TAG_DIRECT: &str = "direct";
pub const TAG_BLOCK: &str = "block";

// 业务分流组（可选，但对大多数用户比较实用）
pub const TAG_TELEGRAM: &str = "Telegram";
pub const TAG_YOUTUBE: &str = "YouTube";
pub const TAG_NETFLIX: &str = "Netflix";
pub const TAG_OPENAI: &str = "OpenAI";
pub const TAG_GOOGLE: &str = "Google";

// DNS server tags
pub const DNS_PROXY: &str = "dns_proxy";
pub const DNS_CN: &str = "dns_cn";
pub const DNS_RESOLVER: &str = "dns_resolver";
pub const DNS_FAKEIP: &str = "dns_fakeip";

pub const FAKE_DNS_FILTER_PROXY_ONLY: &str = "proxy_only";
pub const FAKE_DNS_FILTER_GLOBAL_NON_CN: &str = "global_non_cn";

// Rule-set tags (官方 SagerNet 规则集)
pub const RS_GEOSITE_CN: &str = "geosite-cn";
pub const RS_GEOSITE_GEOLOCATION_NOT_CN: &str = "geosite-geolocation-!cn";
pub const RS_GEOSITE_PRIVATE: &str = "geosite-private";
pub const RS_GEOSITE_ADS: &str = "geosite-category-ads-all";
pub const RS_GEOSITE_TELEGRAM: &str = "geosite-telegram";
pub const RS_GEOSITE_YOUTUBE: &str = "geosite-youtube";
pub const RS_GEOSITE_NETFLIX: &str = "geosite-netflix";
pub const RS_GEOSITE_OPENAI: &str = "geosite-openai";
pub const RS_GEOSITE_GOOGLE: &str = "geosite-google";
pub const RS_GEOIP_CN: &str = "geoip-cn";
pub const RS_GEOIP_PRIVATE: &str = "geoip-private";
pub const PRIVATE_IP_CIDRS: &[&str] = &[
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "::1/128",
    "fc00::/7",
    "fe80::/10",
];

/// Telegram 官方 DC / API 网段。桌面客户端直连这些 IP，不走域名，
/// 只有 geosite-telegram 会漏掉，TUN 下表现为 SYN_SENT、App 一直「正在连接」。
pub const TELEGRAM_DC_IP_CIDRS: &[&str] = &[
    "91.108.4.0/22",
    "91.108.8.0/22",
    "91.108.12.0/22",
    "91.108.16.0/22",
    "91.108.20.0/22",
    "91.108.56.0/22",
    "149.154.160.0/20",
    "185.76.151.0/24",
    "2001:67c:4e8::/48",
    "2001:b28:f23c::/48",
    "2001:b28:f23d::/48",
    "2001:b28:f23f::/48",
];

pub fn kernel_log_output_path() -> String {
    paths::get_kernel_work_dir()
        .join("sing-box.log")
        .to_string_lossy()
        .to_string()
}

pub fn ensure_kernel_log_output(config_obj: &mut Map<String, Value>) {
    let log = config_obj.entry("log".to_string()).or_insert(json!({}));
    if !log.is_object() {
        *log = json!({});
    }

    let Some(log_obj) = log.as_object_mut() else {
        return;
    };

    log_obj
        .entry("disabled".to_string())
        .or_insert(json!(false));
    log_obj.entry("level".to_string()).or_insert(json!("info"));
    log_obj
        .entry("timestamp".to_string())
        .or_insert(json!(true));
    log_obj.insert("output".to_string(), json!(kernel_log_output_path()));
}

/// urltest 测速间隔。30s 扫一遍 100+ 节点会把空闲带宽打满，体感就是网页一顿一顿。
pub const URLTEST_INTERVAL: &str = "5m";
/// 切换容差（毫秒）。香港家宽彼此只差几十毫秒，过小会来回切。
pub const URLTEST_TOLERANCE: u64 = 400;
/// 空闲超过此时长停止测速，避免后台一直打测速包。
pub const URLTEST_IDLE_TIMEOUT: &str = "30m";
/// 自动组最多探测这么多条。全量探测又慢又抖。
pub const URLTEST_MAX_CANDIDATES: usize = 8;

/// 订阅里常见的「提示/官网/流量」占位节点，放进 urltest 会变成启动即死路。
const PLACEHOLDER_MARKERS: &[&str] = &[
    "官网",
    "流量",
    "过期",
    "到期",
    "剩余",
    "到期时间",
    "yuntijiasu",
    "yunti.io",
];

pub fn is_placeholder_node_tag(tag: &str) -> bool {
    PLACEHOLDER_MARKERS.iter().any(|marker| tag.contains(marker))
}

fn is_hong_kong(tag: &str) -> bool {
    let upper = tag.to_ascii_uppercase();
    tag.contains("香港") || tag.contains("🇭🇰") || upper.contains("HK")
}

fn is_residential(tag: &str) -> bool {
    tag.contains("家宽") || tag.contains("住宅") || tag.contains("IEPL") || tag.contains("IPLC")
}

/// 越小越优先进入自动组。
/// 实测：香港家宽吞吐 90–120Mbps，轻量节点延迟低但只有 20Mbps。
fn quality_rank(tag: &str) -> u8 {
    match (is_hong_kong(tag), is_residential(tag)) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    }
}

fn is_reserved_outbound_tag(tag: &str) -> bool {
    matches!(
        tag,
        TAG_DIRECT | TAG_AUTO | TAG_MANUAL | TAG_BLOCK | TAG_TELEGRAM | TAG_YOUTUBE | TAG_NETFLIX
            | TAG_OPENAI | TAG_GOOGLE
    )
}

/// 从订阅节点里挑自动选优候选：丢掉占位节点，香港家宽优先，控制探测规模。
pub fn select_urltest_candidates(tags: &[String]) -> Vec<String> {
    let mut usable: Vec<&String> = tags
        .iter()
        .filter(|tag| !is_reserved_outbound_tag(tag) && !is_placeholder_node_tag(tag))
        .collect();

    usable.sort_by(|a, b| {
        quality_rank(a)
            .cmp(&quality_rank(b))
            .then_with(|| a.cmp(b))
    });

    // 有香港就只跑香港：远端延迟高，塞进自动组会把测速和切换一起拖慢。
    if usable.iter().any(|tag| is_hong_kong(tag)) {
        usable.retain(|tag| is_hong_kong(tag));
    }

    let picked: Vec<String> = usable
        .into_iter()
        .take(URLTEST_MAX_CANDIDATES)
        .cloned()
        .collect();

    if picked.is_empty() {
        vec![TAG_DIRECT.to_string()]
    } else {
        picked
    }
}

pub fn urltest_outbound_template(url: &str, outbounds: Vec<Value>) -> Value {
    json!({
        "type": "urltest",
        "tag": TAG_AUTO,
        "outbounds": outbounds,
        // 容差拉大后很少切节点；切的时候不断旧连接，避免网页/视频被集体掐断。
        "interrupt_exist_connections": false,
        "idle_timeout": URLTEST_IDLE_TIMEOUT,
        "interval": URLTEST_INTERVAL,
        "tolerance": URLTEST_TOLERANCE,
        "url": url,
    })
}

pub fn apply_urltest_stability_settings(obj: &mut Map<String, Value>, url: &str) {
    obj.insert("interrupt_exist_connections".to_string(), json!(false));
    obj.insert("idle_timeout".to_string(), json!(URLTEST_IDLE_TIMEOUT));
    obj.insert("interval".to_string(), json!(URLTEST_INTERVAL));
    obj.insert("tolerance".to_string(), json!(URLTEST_TOLERANCE));
    if !url.is_empty() {
        obj.insert("url".to_string(), json!(url));
    }
    if let Some(members) = obj.get("outbounds").and_then(|value| value.as_array()) {
        let tags: Vec<String> = members
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect();
        if !tags.is_empty() {
            let selected = select_urltest_candidates(&tags);
            obj.insert(
                "outbounds".to_string(),
                Value::Array(selected.into_iter().map(Value::String).collect()),
            );
        }
    }
}

pub fn normalize_default_outbound(app_config: &AppConfig) -> &'static str {
    match app_config.singbox_default_proxy_outbound.as_str() {
        "auto" => TAG_AUTO,
        _ => TAG_MANUAL,
    }
}

pub fn normalize_download_detour(app_config: &AppConfig) -> &'static str {
    match app_config.singbox_download_detour.as_str() {
        "manual" => TAG_MANUAL,
        // 默认值调整为直连：gh-proxy 已经加速，避免多余的代理链路
        _ => TAG_DIRECT,
    }
}

pub fn dns_strategy(app_config: &AppConfig) -> &'static str {
    if app_config.prefer_ipv6 {
        "prefer_ipv6"
    } else {
        "ipv4_only"
    }
}

pub fn node_domain_resolver_strategy(app_config: &AppConfig) -> &'static str {
    if app_config.prefer_ipv6 {
        "prefer_ipv6"
    } else {
        // 节点域名解析默认走 IPv4，能显著降低“有 AAAA 但本机 IPv6 不可用”导致的连接失败。
        "ipv4_only"
    }
}

pub fn normalize_fake_dns_filter_mode(app_config: &AppConfig) -> &'static str {
    match app_config.singbox_fake_dns_filter_mode.as_str() {
        FAKE_DNS_FILTER_GLOBAL_NON_CN => FAKE_DNS_FILTER_GLOBAL_NON_CN,
        _ => FAKE_DNS_FILTER_PROXY_ONLY,
    }
}

fn default_dns_server_port(server_type: &str) -> u16 {
    match server_type {
        "https" | "h3" | "tls" | "quic" => 443,
        _ => 53,
    }
}

fn is_domain(value: &str) -> bool {
    value.parse::<IpAddr>().is_err()
}

fn parse_legacy_host_port(input: &str) -> Result<(String, Option<u16>), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("DNS 服务器地址为空".to_string());
    }

    if trimmed.starts_with('[') {
        // 支持 [IPv6]:port 形式
        if let Some(end) = trimmed.find(']') {
            let host = &trimmed[1..end];
            if host.is_empty() {
                return Err("DNS IPv6 地址为空".to_string());
            }
            let port = if end + 1 < trimmed.len() {
                let suffix = &trimmed[end + 1..];
                if let Some(port_str) = suffix.strip_prefix(':') {
                    Some(
                        port_str
                            .parse::<u16>()
                            .map_err(|_| format!("无效的 DNS 端口: {}", port_str))?,
                    )
                } else {
                    return Err(format!("无效的 DNS 地址格式: {}", trimmed));
                }
            } else {
                None
            };
            return Ok((host.to_string(), port));
        }
    }

    // 裸 IPv6（不带端口）直接作为 host 使用
    if trimmed.matches(':').count() > 1 && trimmed.parse::<IpAddr>().is_ok() {
        return Ok((trimmed.to_string(), None));
    }

    if let Some((host, port)) = trimmed.rsplit_once(':') {
        if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return Ok((
                host.to_string(),
                Some(
                    port.parse::<u16>()
                        .map_err(|_| format!("无效的 DNS 端口: {}", port))?,
                ),
            ));
        }
    }

    Ok((trimmed.to_string(), None))
}

/// 将旧 `address` 语义转换为 sing-box 1.12+ 的 DNS 服务器新格式。
///
/// 说明：
/// - 当 `address` 是域名且 `resolver_tag` 非空时，会自动写入 `domain_resolver`（对象格式）。
/// - 仅转换本项目会用到的常见类型：udp/tcp/tls/https/h3/quic/local/dhcp。
pub(crate) fn build_dns_server_config(
    tag: &str,
    address: &str,
    strategy: Option<&str>,
    detour: Option<&str>,
    resolver_tag: Option<&str>,
) -> Result<DnsServerConfig, String> {
    let raw = address.trim();
    if raw.is_empty() {
        return Err(format!("DNS 服务器地址为空: tag={}", tag));
    }

    // 新版 sing-box 中，DNS server 默认就是 direct dial。
    // 显式设置 detour=direct 会触发 "detour to an empty direct outbound makes no sense"。
    let normalized_detour = detour.and_then(|d| {
        let trimmed = d.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(TAG_DIRECT) {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    if raw.eq_ignore_ascii_case("local") {
        return Ok(DnsServerConfig {
            tag: tag.to_string(),
            server_type: Some("local".to_string()),
            server: None,
            server_port: None,
            path: None,
            interface: None,
            inet4_range: None,
            inet6_range: None,
            domain_resolver: None,
            detour: None,
        });
    }

    let mut server_type = "udp".to_string();
    let mut server: Option<String> = None;
    let mut server_port: Option<u16> = None;
    let mut path: Option<String> = None;
    let mut interface: Option<String> = None;

    if raw.contains("://") {
        if raw.starts_with("dhcp://") {
            server_type = "dhcp".to_string();
            let value = raw.trim_start_matches("dhcp://").trim();
            if !value.is_empty() && !value.eq_ignore_ascii_case("auto") {
                interface = Some(value.to_string());
            }
        } else {
            let url = Url::parse(raw).map_err(|e| format!("无效的 DNS 地址: {} ({})", raw, e))?;
            server_type = match url.scheme() {
                "https" => "https".to_string(),
                "h3" => "h3".to_string(),
                "quic" => "quic".to_string(),
                "tls" => "tls".to_string(),
                "tcp" => "tcp".to_string(),
                "udp" => "udp".to_string(),
                unknown => {
                    return Err(format!(
                        "不支持的 DNS 协议: {} (tag={}, address={})",
                        unknown, tag, raw
                    ))
                }
            };

            let host = url
                .host_str()
                .ok_or_else(|| format!("DNS 地址缺少主机: {}", raw))?;
            server = Some(host.to_string());
            server_port = Some(url.port().unwrap_or(default_dns_server_port(&server_type)));

            if matches!(server_type.as_str(), "https" | "h3") {
                let mut p = url.path().to_string();
                if p.is_empty() || p == "/" {
                    p = "/dns-query".to_string();
                }
                path = Some(p);
            }
        }
    } else {
        let (host, port) = parse_legacy_host_port(raw)?;
        server = Some(host);
        server_port = Some(port.unwrap_or(53));
    }

    let mut domain_resolver = None;
    if let (Some(host), Some(resolver)) = (server.as_deref(), resolver_tag) {
        if !resolver.is_empty() && is_domain(host) && tag != resolver {
            domain_resolver = Some(match strategy {
                Some(s) if !s.trim().is_empty() => json!({
                    "server": resolver,
                    "strategy": s
                }),
                _ => json!({
                    "server": resolver
                }),
            });
        }
    }

    Ok(DnsServerConfig {
        tag: tag.to_string(),
        server_type: Some(server_type),
        server,
        server_port,
        path,
        interface,
        inet4_range: None,
        inet6_range: None,
        domain_resolver,
        detour: normalized_detour,
    })
}

#[cfg(test)]
#[path = "common.tests.rs"]
mod tests;
