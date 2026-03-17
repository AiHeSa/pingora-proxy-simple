//! Pingora Proxy - 替代 nginx 的反向代理服务器

use async_trait::async_trait;
use pingora::prelude::*;
use pingora::server::configuration::ServerConf;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_proxy::{ProxyHttp, Session};
use pingora::ErrorType;
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

// ==================== 配置结构 ====================

#[derive(Debug, Deserialize, Clone)]
struct Config {
    servers: Vec<ServerConfig>,
}

#[derive(Debug, Deserialize, Clone)]
struct ServerConfig {
    listen: String,
    name: String,
    locations: Vec<LocationConfig>,
}

#[derive(Debug, Deserialize, Clone)]
struct LocationConfig {
    path: String,
    upstream: Option<String>,
    static_root: Option<String>,
}

// ==================== 代理服务 ====================

pub struct MyProxy {
    config: Arc<Config>,
}

impl MyProxy {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
    
    fn get_listen_port(listen: &str) -> u16 {
        listen.split(':').last().and_then(|p| p.parse().ok()).unwrap_or(80)
    }
    
    fn get_server_by_port(&self, port: u16) -> Option<&ServerConfig> {
        self.config.servers.iter().find(|s| {
            Self::get_listen_port(&s.listen) == port
        })
    }
    
    fn get_request_port(session: &Session) -> u16 {
        if let Some(host) = session.req_header().headers.get("host") {
            if let Ok(host_str) = host.to_str() {
                if let Some(port_str) = host_str.split(':').last() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        return port;
                    }
                }
            }
        }
        80
    }
    
    // 优先查找 upstream（如 /api），找不到再找静态文件
    fn find_upstream_for_port(&self, path: &str, port: u16) -> Option<String> {
        let server = self.get_server_by_port(port)?;
        // 先匹配更长的路径（确保 /api 优先于 /）
        let mut locations: Vec<_> = server.locations.iter().collect();
        locations.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
        
        for location in locations {
            if path.starts_with(&location.path) {
                if let Some(upstream) = &location.upstream {
                    return Some(upstream.clone());
                }
            }
        }
        None
    }
    
    fn find_static_root_for_port(&self, path: &str, port: u16) -> Option<String> {
        let server = self.get_server_by_port(port)?;
        // 查找没有 upstream 的静态文件配置
        for location in &server.locations {
            if path.starts_with(&location.path) && location.upstream.is_none() {
                if let Some(root) = &location.static_root {
                    return Some(root.clone());
                }
            }
        }
        None
    }
}

#[async_trait]
impl ProxyHttp for MyProxy {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let path = session.req_header().uri.path();
        let port = Self::get_request_port(session);
        
        if let Some(upstream) = self.find_upstream_for_port(path, port) {
            info!("Proxying {} (port {}) -> {}", path, port, upstream);
            let peer = Box::new(HttpPeer::new(&upstream, false, String::new()));
            return Ok(peer);
        }

        Err(Error::explain(
            ErrorType::new("NoUpstream"),
            format!("No upstream for path {} on port {}", path, port),
        ))
    }
    
    async fn request_filter(&self, session: &mut Session, _ctx: &mut Self::CTX) -> Result<bool> {
        let path = session.req_header().uri.path();
        let port = Self::get_request_port(session);
        
        // 先检查是否有 upstream，如果有就不处理静态文件
        if self.find_upstream_for_port(path, port).is_some() {
            return Ok(false);
        }
        
        // 处理静态文件
        if let Some(root) = self.find_static_root_for_port(path, port) {
            let file_path = if path == "/" {
                format!("{}/index.html", root)
            } else {
                format!("{}{}", root, path)
            };
            
            if let Ok(content) = std::fs::read(&file_path) {
                let mime = mime_guess::from_path(&file_path)
                    .first_or_octet_stream()
                    .to_string();
                
                let mut resp = ResponseHeader::build(200, None)?;
                resp.insert_header("Content-Type", &mime)?;
                resp.insert_header("Content-Length", content.len())?;
                
                session.write_response_header(Box::new(resp), true).await?;
                session.write_response_body(Some(content.into()), true).await?;
                
                info!("Served static file: {} (port {})", file_path, port);
                return Ok(true);
            }
            
            // SPA fallback
            let index_path = format!("{}/index.html", root);
            if let Ok(content) = std::fs::read(&index_path) {
                let mut resp = ResponseHeader::build(200, None)?;
                resp.insert_header("Content-Type", "text/html")?;
                resp.insert_header("Content-Length", content.len())?;
                
                session.write_response_header(Box::new(resp), true).await?;
                session.write_response_body(Some(content.into()), true).await?;
                
                return Ok(true);
            }
        }
        
        Ok(false)
    }
}

// ==================== 主函数 ====================

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config_str = std::fs::read_to_string("config.yaml")
        .expect("Failed to read config.yaml");
    let config: Config = serde_yaml::from_str(&config_str)
        .expect("Failed to parse config.yaml");

    info!("Loaded {} server configurations", config.servers.len());
    for server in &config.servers {
        info!("  - {} listening on {}", server.name, server.listen);
    }

    let conf = Arc::new(ServerConf::default());
    let proxy = MyProxy::new(config.clone());
    let mut proxy_service = pingora_proxy::http_proxy_service(&conf, proxy);
    
    // 从配置文件读取监听地址
    for server in &config.servers {
        info!("Adding listener: {}", server.listen);
        proxy_service.add_tcp(&server.listen);
    }
    
    let mut server = Server::new(Opt::default())?;
    server.add_service(proxy_service);

    info!("Starting Pingora proxy server...");
    server.run_forever();
}
