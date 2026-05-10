use reqwest::Client;
use std::time::Duration;

/// 协议版注册专用的指纹客户端构造器
/// 通过精确设置 HTTP 头部模拟真实浏览器指纹
pub struct ImpersonateProvider;

impl ImpersonateProvider {
    /// 创建一个模拟 Chrome 124 最新版的客户端
    /// 通过自定义 headers 注入一致的浏览器头部信息
    pub fn create_chrome_client(proxy_url: Option<&str>) -> Client {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(90))
            .danger_accept_invalid_certs(true)
            .cookie_store(true);

        if let Some(proxy) = proxy_url.filter(|u| !u.trim().is_empty()) {
            if let Ok(p) = reqwest::Proxy::all(proxy) {
                builder = builder.proxy(p);
            }
        }

        // 注入标准的 Chrome 124 Client Hints 和基本头部，确保与 TLS 指纹匹配
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "user-agent",
            crate::openai::constants::DEFAULT_USER_AGENT
                .parse()
                .unwrap(),
        );
        headers.insert(
            "sec-ch-ua",
            "\"Chromium\";v=\"124\", \"Google Chrome\";v=\"124\", \"Not-A.Brand\";v=\"99\""
                .parse()
                .unwrap(),
        );
        headers.insert("sec-ch-ua-mobile", "?0".parse().unwrap());
        headers.insert("sec-ch-ua-platform", "\"Windows\"".parse().unwrap());
        headers.insert("upgrade-insecure-requests", "1".parse().unwrap());
        headers.insert("accept-language", "en-US,en;q=0.9".parse().unwrap());

        builder
            .default_headers(headers)
            .build()
            .expect("无法构建指纹模拟客户端")
    }

    /// 创建一个模拟 Safari 17.4 的客户端
    #[allow(dead_code)]
    pub fn create_safari_client(proxy_url: Option<&str>) -> Client {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .cookie_store(true);

        if let Some(proxy) = proxy_url.filter(|u| !u.trim().is_empty()) {
            if let Ok(p) = reqwest::Proxy::all(proxy) {
                builder = builder.proxy(p);
            }
        }

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("user-agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15".parse().unwrap());
        headers.insert("accept-language", "en-US,en;q=0.9".parse().unwrap());

        builder
            .default_headers(headers)
            .build()
            .expect("无法构建 Safari 指纹模拟客户端")
    }
}
