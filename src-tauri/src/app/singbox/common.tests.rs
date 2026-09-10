use super::{
    apply_urltest_stability_settings, build_dns_server_config, is_placeholder_node_tag,
    select_urltest_candidates, URLTEST_IDLE_TIMEOUT, URLTEST_INTERVAL, URLTEST_MAX_CANDIDATES,
    URLTEST_TOLERANCE,
};
use serde_json::{json, Map};

#[test]
fn should_convert_https_legacy_address_to_new_dns_server() {
    let server = build_dns_server_config(
        "dns_proxy",
        "https://1.1.1.1/dns-query",
        Some("ipv4_only"),
        Some("manual"),
        Some("dns_resolver"),
    )
    .expect("https 地址应可转换");

    assert_eq!(server.server_type.as_deref(), Some("https"));
    assert_eq!(server.server.as_deref(), Some("1.1.1.1"));
    assert_eq!(server.server_port, Some(443));
    assert_eq!(server.path.as_deref(), Some("/dns-query"));
}

#[test]
fn should_set_domain_resolver_for_domain_server() {
    let server = build_dns_server_config(
        "dns_proxy",
        "https://dns.google/dns-query",
        Some("prefer_ipv4"),
        Some("manual"),
        Some("dns_resolver"),
    )
    .expect("域名 DNS 地址应可转换");

    assert_eq!(
        server
            .domain_resolver
            .as_ref()
            .and_then(|v| v.get("server"))
            .and_then(|v| v.as_str()),
        Some("dns_resolver")
    );
    assert_eq!(
        server
            .domain_resolver
            .as_ref()
            .and_then(|v| v.get("strategy"))
            .and_then(|v| v.as_str()),
        Some("prefer_ipv4")
    );
}

#[test]
fn should_omit_direct_detour_for_dns_server() {
    let server = build_dns_server_config(
        "dns_resolver",
        "223.5.5.5",
        Some("prefer_ipv4"),
        Some("direct"),
        None,
    )
    .expect("dns_resolver 构建应成功");

    assert!(server.detour.is_none());
}

#[test]
fn placeholder_node_tags_should_be_detected() {
    assert!(is_placeholder_node_tag("PRO-B-官网yuntijiasu.com|v202605"));
    assert!(is_placeholder_node_tag("剩余流量 128GB"));
    assert!(!is_placeholder_node_tag("🇭🇰 PRO-A-香港-HK07-家宽|v202605"));
}

#[test]
fn urltest_candidates_prefer_hk_residential_and_cap_size() {
    let tags: Vec<String> = [
        "🇩🇪 德国-DE02【优化】",
        "🇺🇸 美国-US01【优化】",
        "🇭🇰 PRO-A-香港-HK03|v202605",
        "🇭🇰 PRO-A-香港-HK07-家宽|v202605",
        "🇭🇰 PRO-A-香港-HK09-家宽|v202605",
        "🇭🇰 PRO-A-香港-HK10-家宽|v202605",
        "🇭🇰 旗舰-香港-HK05-家宽|v202605",
        "🇹🇼 PRO-B-台湾-TW01|v202605",
        "PRO-B-官网yuntijiasu.com|v202605",
        "🇯🇵 PRO-C-日本-JP01|v202605",
        "🇸🇬 PRO-F-新加坡-SG01|v202605",
        "🇰🇷 PRO-E-韩国-KR01|v202605",
        "🇬🇧 英国-UK01",
        "🇭🇰 轻量-香港-HK03|v202605",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    let picked = select_urltest_candidates(&tags);
    assert!(picked.len() <= URLTEST_MAX_CANDIDATES);
    assert_eq!(picked.len(), 6);
    assert!(!picked.iter().any(|tag| tag.contains("官网")));
    assert!(
        picked.iter().all(|tag| tag.contains("香港")),
        "自动组应只留香港: {picked:?}"
    );
    assert!(
        picked.iter().take(4).all(|tag| tag.contains("家宽")),
        "前几条应是香港家宽: {picked:?}"
    );
    assert!(!picked.iter().any(|tag| tag.contains("台湾") || tag.contains("德国")));
}

#[test]
fn apply_urltest_stability_settings_should_shrink_bloated_group() {
    let mut obj = Map::new();
    obj.insert(
        "outbounds".to_string(),
        json!([
            "🇩🇪 德国-DE02【优化】",
            "🇺🇸 美国-US01【优化】",
            "🇭🇰 PRO-A-香港-HK07-家宽|v202605",
            "🇹🇼 PRO-B-台湾-TW01|v202605",
            "PRO-B-官网yuntijiasu.com|v202605",
            "🇯🇵 PRO-C-日本-JP01|v202605"
        ]),
    );

    apply_urltest_stability_settings(&mut obj, "http://cp.cloudflare.com/generate_204");

    assert_eq!(obj.get("interval").and_then(|v| v.as_str()), Some(URLTEST_INTERVAL));
    assert_eq!(obj.get("idle_timeout").and_then(|v| v.as_str()), Some(URLTEST_IDLE_TIMEOUT));
    assert_eq!(
        obj.get("tolerance").and_then(|v| v.as_u64()),
        Some(URLTEST_TOLERANCE)
    );
    assert_eq!(
        obj.get("interrupt_exist_connections").and_then(|v| v.as_bool()),
        Some(false)
    );
    let members = obj
        .get("outbounds")
        .and_then(|v| v.as_array())
        .expect("urltest outbounds");
    assert!(members.len() <= URLTEST_MAX_CANDIDATES);
    assert!(!members.iter().any(|v| v.as_str().unwrap_or("").contains("官网")));
    assert!(members
        .iter()
        .any(|v| v.as_str().unwrap_or("").contains("香港")));
}
