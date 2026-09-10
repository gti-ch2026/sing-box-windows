//! 兼容本机残留的旧代理端口（例如 Shadowrocket 的 1082）。
//!
//! 客户机器上常见这种情况：
//! - 浏览器看 `networksetup`，已经是 Pika 的 12080
//! - Telegram 走 TUN
//! - Agent / CLI 进程启动时把 `HTTP_PROXY=http://127.0.0.1:1082` 写进了环境
//!
//! 关掉小火箭节点后 1082 没人听，Agent 就全挂。产品要求「点一下全机走 Pika」，
//! 所以只要我们自己的 mixed inbound 在听，就把 1082 转接到 12080。
//! 客户不需要改 shell、也不需要给某个 App 单独配代理。

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::OnceLock;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

const LEGACY_PORTS: [u16; 1] = [1082];

static COMPAT_ENABLED: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);
static TARGET_PORT: AtomicU16 = AtomicU16::new(12080);
static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

pub fn start_compat_proxy(our_port: u16) {
    if our_port == 0 {
        return;
    }
    TARGET_PORT.store(our_port, Ordering::Relaxed);
    COMPAT_ENABLED.store(true, Ordering::Relaxed);
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }

    let handle = RUNTIME
        .get_or_init(|| {
            tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .thread_name("pika-compat-proxy")
                    .build()
                    .expect("compat proxy runtime");
                let handle = rt.handle().clone();
                std::thread::Builder::new()
                    .name("pika-compat-proxy-rt".into())
                    .spawn(move || {
                        rt.block_on(std::future::pending::<()>());
                    })
                    .expect("compat proxy runtime thread");
                handle
            })
        })
        .clone();

    for port in LEGACY_PORTS {
        if port == our_port {
            continue;
        }
        handle.spawn(accept_loop(port));
    }
}

pub fn stop_compat_proxy() {
    COMPAT_ENABLED.store(false, Ordering::Relaxed);
    STARTED.store(false, Ordering::Relaxed);
}

async fn accept_loop(listen_port: u16) {
    let bind_addrs: [SocketAddr; 2] = [
        SocketAddr::from(([127, 0, 0, 1], listen_port)),
        SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], listen_port)),
    ];

    for addr in bind_addrs {
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                info!(
                    "兼容转发已在 {} 监听，转到 127.0.0.1:{}",
                    addr,
                    TARGET_PORT.load(Ordering::Relaxed)
                );
                tokio::spawn(serve_listener(listener, listen_port));
            }
            Err(err) => {
                if err.kind() == io::ErrorKind::AddrInUse {
                    info!(
                        "{} 仍被其他进程占用，启动旁路转发到 127.0.0.1:{}",
                        addr,
                        TARGET_PORT.load(Ordering::Relaxed)
                    );
                    tokio::spawn(steal_when_free(addr, listen_port));
                } else {
                    warn!("兼容转发绑定 {} 失败: {}", addr, err);
                }
            }
        }
    }
}

async fn steal_when_free(addr: SocketAddr, listen_port: u16) {
    loop {
        if !COMPAT_ENABLED.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if !COMPAT_ENABLED.load(Ordering::Relaxed) {
            return;
        }
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                info!(
                    "兼容转发已接管 {}，转到 127.0.0.1:{}",
                    addr,
                    TARGET_PORT.load(Ordering::Relaxed)
                );
                serve_listener(listener, listen_port).await;
                return;
            }
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => continue,
            Err(err) => {
                warn!("兼容转发重试绑定 {} 失败: {}", addr, err);
                return;
            }
        }
    }
}

async fn serve_listener(listener: TcpListener, listen_port: u16) {
    loop {
        if !COMPAT_ENABLED.load(Ordering::Relaxed) {
            return;
        }
        match listener.accept().await {
            Ok((inbound, _)) => {
                if !COMPAT_ENABLED.load(Ordering::Relaxed) {
                    return;
                }
                tokio::spawn(forward_one(inbound, listen_port));
            }
            Err(err) => {
                if !COMPAT_ENABLED.load(Ordering::Relaxed) {
                    return;
                }
                warn!("兼容转发 accept 失败 ({}): {}", listen_port, err);
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
    }
}

async fn forward_one(mut inbound: TcpStream, listen_port: u16) {
    let target = TARGET_PORT.load(Ordering::Relaxed);
    if target == 0 || target == listen_port {
        return;
    }
    let dest = SocketAddr::from(([127, 0, 0, 1], target));
    match TcpStream::connect(dest).await {
        Ok(mut outbound) => {
            let _ = copy_bidirectional(&mut inbound, &mut outbound).await;
        }
        Err(err) => {
            warn!("兼容转发连接 127.0.0.1:{} 失败: {}", target, err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LEGACY_PORTS;

    #[test]
    fn legacy_ports_include_shadowrocket_http() {
        assert!(LEGACY_PORTS.contains(&1082));
    }
}
