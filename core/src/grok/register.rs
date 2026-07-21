use crate::db::DataLake;
use chrono::{Datelike, Utc};
use headless_chrome::{Browser, LaunchOptions, Tab};
use percent_encoding::percent_decode_str;
use rand::{Rng, distributions::Alphanumeric};
use regex::Regex;
use reqwest::cookie::{CookieStore, Jar};
use reqwest::redirect::Policy;
use reqwest::{Client, Proxy, Url};
use serde_json::{Map, Value, json};
use std::collections::{HashSet, VecDeque};
use std::ffi::OsStr;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

const SIGNUP_URL: &str = "https://accounts.x.ai/sign-up?redirect=grok-com";
const DEFAULT_STATE_TREE: &str = r#"["",{"children":["(app)",{"children":["(auth)",{"children":["sign-up",{"children":["__PAGE__",{},"/sign-up","refresh"]}]},null,null]},null,null]},null,null,true]"#;
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

static SCRIPT_SRC_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<script[^>]+src=["']([^"']+)["']"#).expect("script src regex")
});
static SITE_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)(?:sitekey|data-sitekey)[^0-9]{0,80}(0x4[A-Za-z0-9_-]+)"#)
        .expect("Turnstile site key regex")
});
static ACTION_ID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:createServerReference\)\(\s*["'](7f[a-f0-9]{40})["']|(7f[a-f0-9]{40}))"#)
        .expect("Next action id regex")
});
static SSO_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)https://[^"\s\\]+set-cookie\?q=[^"\s\\]+"#).expect("SSO redirect regex")
});
static AUTH_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)https://auth\.(?:x\.ai|grok\.com|grokipedia\.com)[^"\s\\]+"#)
        .expect("auth redirect regex")
});

pub type StepCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;

#[derive(Clone)]
pub struct GrokRegisterContext {
    pub email: String,
    pub password: String,
    pub proxy_url: Option<String>,
    pub captcha_key: Option<String>,
    pub turnstile_solver_url: Option<String>,
    pub headless: bool,
    pub timeout_secs: u64,
    pub run_id: String,
    pub step_callback: Option<StepCallback>,
}

pub struct GrokRegisterResult {
    pub email: String,
    pub password: String,
    pub sso: String,
}

#[derive(Clone)]
struct SignupParameters {
    site_key: String,
    action_id: String,
}

struct ProtocolSession {
    client: Client,
    jar: Arc<Jar>,
}

struct ChromiumProxyConfig {
    server: String,
    credentials: Option<(String, String)>,
}

pub async fn execute_registration(
    dl: &Arc<DataLake>,
    context: &GrokRegisterContext,
) -> Result<GrokRegisterResult, String> {
    check_cancelled(dl, &context.run_id).await?;
    log(context, "info", "正在初始化 Grok/xAI 注册协议会话");

    let session = build_session(context)?;
    let parameters = match discover_signup_parameters(&session.client).await {
        Ok(parameters) => parameters,
        Err(protocol_error) => {
            log(
                context,
                "warn",
                &format!("协议方式发现注册参数失败，正在尝试 Chromium 回退: {protocol_error}"),
            );
            discover_signup_parameters_browser(context)
                .await
                .map_err(|browser_error| {
                    format!(
                        "xAI 注册参数发现失败；协议错误: {protocol_error}；Chromium 错误: {browser_error}"
                    )
                })?
        }
    };
    log(context, "success", "已发现 xAI 动态注册参数");

    check_cancelled(dl, &context.run_id).await?;
    let requested_at = Utc::now().timestamp().saturating_sub(5);
    send_email_code(&session.client, &context.email).await?;
    log(
        context,
        "info",
        "xAI 验证邮件已请求，正在等待邮件中枢接收验证码",
    );

    let code = wait_for_email_code(dl, context, requested_at).await?;
    log(context, "success", "已取得 xAI 验证码，正在校验");
    verify_email_code(&session.client, &context.email, &code).await?;

    check_cancelled(dl, &context.run_id).await?;
    let turnstile_token = solve_turnstile(context, &parameters.site_key).await?;
    log(context, "success", "Turnstile 验证已完成");

    let (given_name, family_name) = random_name();
    let payload = json!({
        "emailValidationCode": code,
        "createUserAndSessionRequest": {
            "email": context.email,
            "givenName": given_name,
            "familyName": family_name,
            "clearTextPassword": context.password,
            "tosAcceptedVersion": "$undefined"
        },
        "turnstileToken": turnstile_token,
        "promptOnDuplicateEmail": true
    });

    log(context, "info", "正在提交 xAI 账号资料");
    let response_body = submit_signup(&session.client, &parameters, payload).await?;

    check_cancelled(dl, &context.run_id).await?;
    let sso = extract_sso(&session, &response_body).await?;
    log(context, "success", "Grok SSO 会话凭证已提取");

    post_registration_init(&session.client, &sso, context).await;

    Ok(GrokRegisterResult {
        email: context.email.clone(),
        password: context.password.clone(),
        sso,
    })
}

fn build_session(context: &GrokRegisterContext) -> Result<ProtocolSession, String> {
    let jar = Arc::new(Jar::default());
    let timeout = context.timeout_secs.clamp(60, 600);
    let mut builder = Client::builder()
        .cookie_provider(Arc::clone(&jar))
        .redirect(Policy::limited(10))
        .timeout(Duration::from_secs(timeout))
        .connect_timeout(Duration::from_secs(20))
        .user_agent(DEFAULT_USER_AGENT);

    if let Some(proxy_url) = context
        .proxy_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let proxy = Proxy::all(proxy_url).map_err(|error| format!("Grok 代理地址无效: {error}"))?;
        builder = builder.proxy(proxy);
    }

    let client = builder
        .build()
        .map_err(|error| format!("Grok HTTP 会话初始化失败: {error}"))?;
    Ok(ProtocolSession { client, jar })
}

async fn discover_signup_parameters(client: &Client) -> Result<SignupParameters, String> {
    let response = client
        .get(SIGNUP_URL)
        .header(
            "accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("accept-language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|error| format!("访问 xAI 注册页失败: {error}"))?;
    let status = response.status();
    let html = response
        .text()
        .await
        .map_err(|error| format!("读取 xAI 注册页失败: {error}"))?;
    reject_blocked_response(status.as_u16(), &html, "注册页参数发现")?;

    let mut site_key = find_site_key(&html).unwrap_or_default();
    let mut action_id = find_action_id(&html).unwrap_or_default();
    let base = Url::parse(SIGNUP_URL).map_err(|error| error.to_string())?;

    let script_urls = SCRIPT_SRC_REGEX
        .captures_iter(&html)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_string()))
        .filter(|src| src.contains("/_next/static"))
        .take(48)
        .collect::<Vec<_>>();

    for src in script_urls {
        if !site_key.is_empty() && !action_id.is_empty() {
            break;
        }
        let Ok(url) = base.join(&src) else {
            continue;
        };
        let Ok(response) = client.get(url).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(script) = response.text().await else {
            continue;
        };
        if site_key.is_empty() {
            site_key = find_site_key(&script).unwrap_or_default();
        }
        if action_id.is_empty() {
            action_id = find_action_id(&script).unwrap_or_default();
        }
    }

    if site_key.is_empty() || action_id.is_empty() {
        return Err(format!(
            "xAI 注册页缺少动态参数（site_key={}, action_id={}），可能遭遇 Cloudflare 拦截或页面协议已更新",
            !site_key.is_empty(),
            !action_id.is_empty()
        ));
    }

    Ok(SignupParameters {
        site_key,
        action_id,
    })
}

async fn discover_signup_parameters_browser(
    context: &GrokRegisterContext,
) -> Result<SignupParameters, String> {
    let proxy = context.proxy_url.clone().unwrap_or_default();
    let headless = context.headless;
    let timeout_secs = context.timeout_secs.clamp(45, 180);

    tokio::task::spawn_blocking(move || {
        let mut args = vec![
            "--disable-blink-features=AutomationControlled".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--disable-infobars".to_string(),
            "--window-position=0,0".to_string(),
            "--window-size=1280,900".to_string(),
            format!("--user-agent={DEFAULT_USER_AGENT}"),
        ];
        let proxy_config = chromium_proxy_config(&proxy)?;
        let proxy_auth = proxy_config
            .as_ref()
            .and_then(|config| config.credentials.clone());
        if let Some(config) = proxy_config {
            args.push(format!("--proxy-server={}", config.server));
        }

        let options = LaunchOptions::default_builder()
            .headless(headless)
            .sandbox(crate::chromium::sandbox_enabled())
            .window_size(Some((1280, 900)))
            .idle_browser_timeout(Duration::from_secs(timeout_secs + 30))
            .args(args.iter().map(|value| OsStr::new(value)).collect())
            .build()
            .map_err(|error| format!("Chromium 参数无效: {error}"))?;
        let browser = Browser::new(options)
            .map_err(|error| format!("无法启动 Chromium 参数发现回退: {error}"))?;
        let tab = browser
            .new_tab()
            .map_err(|error| format!("无法创建 Chromium 参数发现页面: {error}"))?;
        enable_chromium_proxy_auth(&tab, proxy_auth, "Chromium 参数发现")?;
        tab.navigate_to(SIGNUP_URL)
            .map_err(|error| format!("Chromium 无法打开 xAI 注册页: {error}"))?;
        tab.wait_until_navigated()
            .map_err(|error| format!("Chromium 等待 xAI 注册页失败: {error}"))?;

        let collected = tab
            .evaluate(
                r#"(async () => {
                    const chunks = [String(document.documentElement?.outerHTML || '').slice(0, 2_000_000)];
                    const urls = [...document.scripts]
                        .map(script => script.src)
                        .filter(Boolean)
                        .filter(value => {
                            try {
                                const url = new URL(value, location.href);
                                return url.origin === location.origin && url.pathname.includes('/_next/static/');
                            } catch (_) { return false; }
                        })
                        .slice(0, 48);
                    for (const url of urls) {
                        try {
                            const response = await fetch(url, { credentials: 'include' });
                            if (response.ok) chunks.push((await response.text()).slice(0, 2_000_000));
                        } catch (_) {}
                    }
                    return chunks.join('\n').slice(0, 16_000_000);
                })()"#,
                true,
            )
            .map_err(|error| format!("Chromium 读取 xAI 动态资源失败: {error}"))?
            .value
            .and_then(|value| value.as_str().map(str::to_string))
            .ok_or_else(|| "Chromium 未返回可解析的 xAI 页面内容".to_string())?;

        let site_key = find_site_key(&collected);
        let action_id = find_action_id(&collected);
        let site_key_found = site_key.is_some();
        let action_id_found = action_id.is_some();
        if let (Some(site_key), Some(action_id)) = (site_key, action_id) {
            return Ok(SignupParameters {
                site_key,
                action_id,
            });
        }
        reject_blocked_response(200, &collected, "Chromium 注册页参数发现")?;
        Err(format!(
            "Chromium 注册页缺少动态参数（site_key={site_key_found}, action_id={action_id_found}），页面协议可能已更新"
        ))
    })
    .await
    .map_err(|error| format!("Chromium 参数发现任务异常: {error}"))?
}

fn find_site_key(source: &str) -> Option<String> {
    SITE_KEY_REGEX
        .captures(source)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

fn find_action_id(source: &str) -> Option<String> {
    ACTION_ID_REGEX.captures(source).and_then(|captures| {
        captures
            .get(1)
            .or_else(|| captures.get(2))
            .map(|value| value.as_str().to_string())
    })
}

fn grpc_message(values: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let bytes = value.as_bytes();
        payload.push((((index + 1) << 3) | 2) as u8);
        encode_varint(bytes.len(), &mut payload);
        payload.extend_from_slice(bytes);
    }

    let mut framed = Vec::with_capacity(payload.len() + 5);
    framed.push(0);
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(&payload);
    framed
}

fn encode_varint(mut value: usize, output: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

async fn send_email_code(client: &Client, email: &str) -> Result<(), String> {
    post_grpc(
        client,
        "https://accounts.x.ai/auth_mgmt.AuthManagement/CreateEmailValidationCode",
        grpc_message(&[email]),
        "发送验证邮件",
    )
    .await
}

async fn verify_email_code(client: &Client, email: &str, code: &str) -> Result<(), String> {
    post_grpc(
        client,
        "https://accounts.x.ai/auth_mgmt.AuthManagement/VerifyEmailValidationCode",
        grpc_message(&[email, code]),
        "校验邮箱验证码",
    )
    .await
}

async fn post_grpc(client: &Client, url: &str, body: Vec<u8>, action: &str) -> Result<(), String> {
    let response = client
        .post(url)
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("x-user-agent", "connect-es/2.1.1")
        .header("origin", "https://accounts.x.ai")
        .header("referer", SIGNUP_URL)
        .body(body)
        .send()
        .await
        .map_err(|error| format!("{action}请求失败: {error}"))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    reject_blocked_response(status.as_u16(), &text, action)
}

async fn wait_for_email_code(
    dl: &Arc<DataLake>,
    context: &GrokRegisterContext,
    requested_at: i64,
) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(context.timeout_secs.clamp(60, 600));
    let mut next_notice = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        check_cancelled(dl, &context.run_id).await?;
        match dl.poll_otp_by_email(&context.email, requested_at).await {
            Ok(Some(code)) => {
                let normalized = code
                    .chars()
                    .filter(|value| value.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_uppercase();
                if normalized.len() == 6 {
                    return Ok(normalized);
                }
            }
            Ok(None) => {}
            Err(error) => return Err(format!("查询 xAI 验证邮件失败: {error}")),
        }

        if Instant::now() >= next_notice {
            log(context, "info", "仍在等待 xAI 验证邮件进入中枢");
            next_notice = Instant::now() + Duration::from_secs(15);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(format!(
        "等待 xAI 验证邮件超时（{} 秒），请检查 account_domain 与 Cloudflare 邮件路由",
        context.timeout_secs.clamp(60, 600)
    ))
}

async fn solve_turnstile(context: &GrokRegisterContext, site_key: &str) -> Result<String, String> {
    let mut failures = Vec::new();
    if let Some(key) = context
        .captcha_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        log(context, "info", "正在通过 YesCaptcha 处理 Turnstile");
        match solve_turnstile_yescaptcha(context, site_key, key).await {
            Ok(token) => return Ok(token),
            Err(error) => {
                log(
                    context,
                    "warn",
                    &format!("YesCaptcha 失败，正在尝试下一种方式: {error}"),
                );
                failures.push(format!("YesCaptcha: {error}"));
            }
        }
    }

    if let Some(solver_url) = context
        .turnstile_solver_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        log(context, "info", "正在通过本地 Solver 处理 Turnstile");
        match solve_turnstile_local(context, site_key, solver_url).await {
            Ok(token) => return Ok(token),
            Err(error) => {
                log(
                    context,
                    "warn",
                    &format!("本地 Solver 失败，正在尝试 Chromium 回退: {error}"),
                );
                failures.push(format!("本地 Solver: {error}"));
            }
        }
    }

    log(
        context,
        "info",
        "未配置外部 Solver，正在使用 Chromium Turnstile 回退",
    );
    match solve_turnstile_browser(context, site_key).await {
        Ok(token) => Ok(token),
        Err(error) if failures.is_empty() => Err(error),
        Err(error) => {
            failures.push(format!("Chromium: {error}"));
            Err(format!(
                "所有 Turnstile 处理方式均失败：{}",
                failures.join("；")
            ))
        }
    }
}

async fn solve_turnstile_yescaptcha(
    context: &GrokRegisterContext,
    site_key: &str,
    client_key: &str,
) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("YesCaptcha 客户端初始化失败: {error}"))?;
    let mut task = Map::new();
    task.insert(
        "type".to_string(),
        Value::String(
            if context
                .proxy_url
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                "TurnstileTask".to_string()
            } else {
                "TurnstileTaskProxyless".to_string()
            },
        ),
    );
    task.insert(
        "websiteURL".to_string(),
        Value::String(SIGNUP_URL.to_string()),
    );
    task.insert(
        "websiteKey".to_string(),
        Value::String(site_key.to_string()),
    );
    if let Some(proxy_url) = context
        .proxy_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        add_yescaptcha_proxy_fields(&mut task, proxy_url)?;
    }

    let response = client
        .post("https://api.yescaptcha.com/createTask")
        .json(&json!({"clientKey": client_key, "task": task}))
        .send()
        .await
        .map_err(|error| format!("YesCaptcha 创建任务失败: {error}"))?;
    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("YesCaptcha 创建任务响应无效: {error}"))?;
    ensure_solver_success(&payload, "YesCaptcha 创建任务")?;
    let task_id = value_as_id(payload.get("taskId"))
        .ok_or_else(|| "YesCaptcha 创建任务未返回 taskId".to_string())?;

    poll_solver_token(
        context.timeout_secs,
        Duration::from_secs(5),
        Duration::from_secs(2),
        || {
            client
                .post("https://api.yescaptcha.com/getTaskResult")
                .json(&json!({"clientKey": client_key, "taskId": task_id.clone()}))
                .send()
        },
        "YesCaptcha",
    )
    .await
}

fn add_yescaptcha_proxy_fields(
    task: &mut Map<String, Value>,
    proxy_url: &str,
) -> Result<(), String> {
    let parsed =
        Url::parse(proxy_url).map_err(|error| format!("代理地址无法用于 YesCaptcha: {error}"))?;
    let host = parsed
        .host_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "代理地址缺少主机名".to_string())?;
    let proxy_type = match parsed.scheme() {
        "socks5" | "socks5h" => "socks5",
        "socks4" | "socks4a" => "socks4",
        _ => "http",
    };
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "代理地址缺少端口".to_string())?;
    task.insert(
        "proxyType".to_string(),
        Value::String(proxy_type.to_string()),
    );
    task.insert("proxyAddress".to_string(), Value::String(host.to_string()));
    task.insert("proxyPort".to_string(), Value::Number(port.into()));
    if !parsed.username().is_empty() {
        task.insert(
            "proxyLogin".to_string(),
            Value::String(parsed.username().to_string()),
        );
    }
    if let Some(password) = parsed.password() {
        task.insert(
            "proxyPassword".to_string(),
            Value::String(password.to_string()),
        );
    }
    Ok(())
}

async fn solve_turnstile_local(
    context: &GrokRegisterContext,
    site_key: &str,
    solver_url: &str,
) -> Result<String, String> {
    let base = validate_solver_url(solver_url)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("本地 Solver 客户端初始化失败: {error}"))?;
    let mut create_url = base
        .join("turnstile")
        .map_err(|error| format!("本地 Solver URL 无效: {error}"))?;
    {
        let mut query = create_url.query_pairs_mut();
        query.append_pair("url", SIGNUP_URL);
        query.append_pair("sitekey", site_key);
        if let Some(proxy) = context
            .proxy_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            query.append_pair("proxy", proxy);
        }
    }
    let payload: Value = client
        .get(create_url)
        .send()
        .await
        .map_err(|error| format!("本地 Solver 创建任务失败: {error}"))?
        .json()
        .await
        .map_err(|error| format!("本地 Solver 创建任务响应无效: {error}"))?;
    let task_id = value_as_id(payload.get("taskId"))
        .ok_or_else(|| "本地 Solver 未返回 taskId".to_string())?;
    let result_url = base
        .join("result")
        .map_err(|error| format!("本地 Solver URL 无效: {error}"))?;

    poll_solver_token(
        context.timeout_secs,
        Duration::from_secs(5),
        Duration::from_secs(2),
        || {
            client
                .get(result_url.clone())
                .query(&[("id", task_id.as_str())])
                .send()
        },
        "本地 Solver",
    )
    .await
}

async fn poll_solver_token<F, Fut>(
    timeout_secs: u64,
    initial_delay: Duration,
    poll_interval: Duration,
    mut request: F,
    provider: &str,
) -> Result<String, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    tokio::time::sleep(initial_delay).await;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs.clamp(45, 180));
    while Instant::now() < deadline {
        let response = request()
            .await
            .map_err(|error| format!("{provider} 查询任务失败: {error}"))?;
        let payload: Value = response
            .json()
            .await
            .map_err(|error| format!("{provider} 查询响应无效: {error}"))?;
        ensure_solver_success(&payload, &format!("{provider} 查询任务"))?;
        if let Some(token) = payload
            .get("solution")
            .and_then(|solution| solution.get("token"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "CAPTCHA_FAIL")
        {
            return Ok(token.to_string());
        }
        tokio::time::sleep(poll_interval).await;
    }
    Err(format!("{provider} 处理 Turnstile 超时"))
}

fn ensure_solver_success(payload: &Value, action: &str) -> Result<(), String> {
    if payload
        .get("errorId")
        .and_then(Value::as_i64)
        .is_some_and(|value| value != 0)
    {
        let detail = payload
            .get("errorDescription")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(format!("{action}失败: {detail}"));
    }
    Ok(())
}

fn value_as_id(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn chromium_proxy_config(value: &str) -> Result<Option<ChromiumProxyConfig>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let mut url = Url::parse(value).map_err(|error| format!("浏览器代理地址无效: {error}"))?;
    if url.host_str().is_none() {
        return Err("浏览器代理地址缺少主机名".to_string());
    }

    let normalized_scheme = match url.scheme() {
        "http" => "http",
        "https" => "https",
        "socks4" | "socks4a" => "socks4",
        "socks5" | "socks5h" => "socks5",
        scheme => return Err(format!("Chromium 不支持代理协议: {scheme}")),
    };

    let credentials = if !url.username().is_empty() || url.password().is_some() {
        if normalized_scheme.starts_with("socks") {
            return Err(
                "Chromium 不支持带用户名和密码的 SOCKS 代理；请改用 HTTP/HTTPS 认证代理或本地无认证转发"
                    .to_string(),
            );
        }
        Some((
            percent_decode_str(url.username())
                .decode_utf8_lossy()
                .into_owned(),
            percent_decode_str(url.password().unwrap_or_default())
                .decode_utf8_lossy()
                .into_owned(),
        ))
    } else {
        None
    };

    url.set_username("")
        .map_err(|_| "无法清理浏览器代理用户名".to_string())?;
    url.set_password(None)
        .map_err(|_| "无法清理浏览器代理密码".to_string())?;
    url.set_scheme(normalized_scheme)
        .map_err(|_| "无法规范化浏览器代理协议".to_string())?;
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);

    Ok(Some(ChromiumProxyConfig {
        server: url.as_str().trim_end_matches('/').to_string(),
        credentials,
    }))
}

fn enable_chromium_proxy_auth(
    tab: &Arc<Tab>,
    credentials: Option<(String, String)>,
    action: &str,
) -> Result<(), String> {
    let Some((username, password)) = credentials else {
        return Ok(());
    };

    let tab_for_auth = Arc::clone(tab);
    tab.add_event_listener(Arc::new(
        move |event: &headless_chrome::protocol::cdp::types::Event| {
            if let headless_chrome::protocol::cdp::types::Event::FetchAuthRequired(auth_event) =
                event
            {
                let _ = tab_for_auth.call_method(
                    headless_chrome::protocol::cdp::Fetch::ContinueWithAuth {
                        request_id: auth_event.params.request_id.clone(),
                        auth_challenge_response:
                            headless_chrome::protocol::cdp::Fetch::AuthChallengeResponse {
                                response: headless_chrome::protocol::cdp::Fetch::AuthChallengeResponseResponse::ProvideCredentials,
                                username: Some(username.clone()),
                                password: Some(password.clone()),
                            },
                    },
                );
            }
        },
    ))
    .map_err(|error| format!("{action}添加代理认证监听器失败: {error}"))?;

    tab.call_method(headless_chrome::protocol::cdp::Fetch::Enable {
        patterns: None,
        handle_auth_requests: Some(true),
    })
    .map_err(|error| format!("{action}启用代理认证失败: {error}"))?;
    Ok(())
}

pub(crate) fn validate_solver_url(value: &str) -> Result<Url, String> {
    let mut url = Url::parse(value).map_err(|error| format!("本地 Solver URL 无效: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("本地 Solver URL 仅支持带主机名的 http/https 地址".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("本地 Solver URL 不允许内嵌用户名或密码".to_string());
    }
    url.set_query(None);
    url.set_fragment(None);
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}

async fn solve_turnstile_browser(
    context: &GrokRegisterContext,
    site_key: &str,
) -> Result<String, String> {
    let site_key = site_key.to_string();
    let proxy = context.proxy_url.clone().unwrap_or_default();
    let headless = context.headless;
    let timeout_secs = context.timeout_secs.clamp(45, 180);

    tokio::task::spawn_blocking(move || {
        let mut args = vec![
            "--disable-blink-features=AutomationControlled".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--disable-infobars".to_string(),
            "--window-position=0,0".to_string(),
            "--window-size=1280,900".to_string(),
            format!("--user-agent={DEFAULT_USER_AGENT}"),
        ];
        let proxy_config = chromium_proxy_config(&proxy)?;
        let proxy_auth = proxy_config
            .as_ref()
            .and_then(|config| config.credentials.clone());
        if let Some(config) = proxy_config {
            args.push(format!("--proxy-server={}", config.server));
        }

        let options = LaunchOptions::default_builder()
            .headless(headless)
            .sandbox(crate::chromium::sandbox_enabled())
            .window_size(Some((1280, 900)))
            .idle_browser_timeout(Duration::from_secs(timeout_secs + 30))
            .args(args.iter().map(|value| OsStr::new(value)).collect())
            .build()
            .map_err(|error| format!("Turnstile 浏览器参数无效: {error}"))?;
        let browser = Browser::new(options)
            .map_err(|error| format!("无法启动 Turnstile Chromium: {error}"))?;
        let tab = browser
            .new_tab()
            .map_err(|error| format!("无法创建 Turnstile 页面: {error}"))?;
        enable_chromium_proxy_auth(&tab, proxy_auth, "Turnstile Chromium")?;
        tab.navigate_to(SIGNUP_URL)
            .map_err(|error| format!("无法打开 xAI Turnstile 页面: {error}"))?;
        std::thread::sleep(Duration::from_secs(3));

        let site_key_json = serde_json::to_string(&site_key).map_err(|error| error.to_string())?;
        let inject = format!(
            r#"(() => {{
                const siteKey = {site_key_json};
                function mount() {{
                    let host = document.getElementById('phantom-grok-turnstile');
                    if (!host) {{
                        host = document.createElement('div');
                        host.id = 'phantom-grok-turnstile';
                        host.style.cssText = 'position:fixed;left:24px;top:24px;z-index:2147483647;background:white;padding:12px';
                        document.body.appendChild(host);
                    }}
                    host.innerHTML = '';
                    if (window.turnstile && turnstile.render) {{
                        turnstile.render(host, {{sitekey: siteKey, theme: 'light'}});
                        return 'rendered';
                    }}
                    return 'waiting';
                }}
                if (window.turnstile) return mount();
                let script = document.querySelector('script[data-phantom-turnstile]');
                if (!script) {{
                    script = document.createElement('script');
                    script.dataset.phantomTurnstile = '1';
                    script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
                    script.async = true;
                    script.onload = mount;
                    document.head.appendChild(script);
                }}
                return 'loading';
            }})()"#
        );
        tab.evaluate(&inject, false)
            .map_err(|error| format!("注入 Turnstile 组件失败: {error}"))?;

        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        while Instant::now() < deadline {
            let result = tab.evaluate(
                r#"(() => {
                    try {
                        if (window.turnstile && turnstile.getResponse) {
                            const token = String(turnstile.getResponse() || '').trim();
                            if (token) return token;
                        }
                        const input = document.querySelector('input[name="cf-turnstile-response"]');
                        return input ? String(input.value || '').trim() : '';
                    } catch (_) { return ''; }
                })()"#,
                false,
            );
            if let Ok(remote) = result {
                if let Some(token) = remote.value.and_then(|value| value.as_str().map(str::to_string)) {
                    if token.len() >= 40 {
                        return Ok(token);
                    }
                }
            }
            let _ = tab.evaluate(
                r#"(() => {
                    const box = document.getElementById('phantom-grok-turnstile');
                    if (!box) return false;
                    box.scrollIntoView({block:'center'});
                    box.dispatchEvent(new MouseEvent('click', {bubbles:true}));
                    return true;
                })()"#,
                false,
            );
            std::thread::sleep(Duration::from_secs(1));
        }
        Err(format!("Chromium Turnstile 回退在 {timeout_secs} 秒内未取得令牌"))
    })
    .await
    .map_err(|error| format!("Turnstile 浏览器任务异常: {error}"))?
}

async fn submit_signup(
    client: &Client,
    parameters: &SignupParameters,
    payload: Value,
) -> Result<String, String> {
    let state_tree =
        url::form_urlencoded::byte_serialize(DEFAULT_STATE_TREE.as_bytes()).collect::<String>();
    let body = serde_json::to_string(&json!([payload]))
        .map_err(|error| format!("构造 xAI 注册请求失败: {error}"))?;
    let response = client
        .post(SIGNUP_URL)
        .header("accept", "text/x-component")
        .header("content-type", "text/plain;charset=UTF-8")
        .header("next-router-state-tree", state_tree)
        .header("next-action", &parameters.action_id)
        .header("origin", "https://accounts.x.ai")
        .header("referer", SIGNUP_URL)
        .body(body)
        .send()
        .await
        .map_err(|error| format!("提交 xAI 注册资料失败: {error}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    reject_blocked_response(status.as_u16(), &body, "提交注册资料")?;

    let lower = body.to_ascii_lowercase();
    if lower.contains("already_exists") || lower.contains("email_in_use") {
        return Err("该邮箱已存在 Grok/xAI 账号".to_string());
    }
    if lower.contains("permission_denied") || lower.contains("invalid_argument") {
        return Err("xAI 拒绝注册资料，Turnstile 或页面动态参数可能已失效".to_string());
    }
    Ok(body)
}

async fn extract_sso(session: &ProtocolSession, response_body: &str) -> Result<String, String> {
    if let Some(sso) = read_sso_cookie(&session.jar) {
        return Ok(sso);
    }

    let unescaped = unescape_rsc(response_body);
    let mut queue = VecDeque::new();
    let mut seen = HashSet::new();
    enqueue_auth_urls(&unescaped, &mut queue, &seen);

    let mut hops = 0usize;
    while let Some(candidate) = queue.pop_front() {
        if hops >= 10 || !seen.insert(candidate.clone()) {
            continue;
        }
        hops += 1;
        if !is_trusted_auth_url(&candidate) {
            continue;
        }
        let Ok(response) = session.client.get(&candidate).send().await else {
            continue;
        };
        if let Some(sso) = read_sso_cookie(&session.jar) {
            return Ok(sso);
        }
        let nested = response.text().await.unwrap_or_default();
        enqueue_auth_urls(&nested, &mut queue, &seen);
    }

    read_sso_cookie(&session.jar).ok_or_else(|| {
        "xAI 注册提交完成，但未取得 SSO Cookie；页面协议可能已变化或登录跳转被网络拦截".to_string()
    })
}

fn unescape_rsc(body: &str) -> String {
    body.replace("\\u0026", "&")
        .replace("\\/", "/")
        .replace("\\u003d", "=")
}

fn enqueue_auth_urls(body: &str, queue: &mut VecDeque<String>, seen: &HashSet<String>) {
    let unescaped = unescape_rsc(body);
    for value in SSO_URL_REGEX
        .find_iter(&unescaped)
        .chain(AUTH_URL_REGEX.find_iter(&unescaped))
    {
        let cleaned = value
            .as_str()
            .trim_end_matches("1:")
            .trim_end_matches(|character: char| {
                matches!(character, ',' | ';' | ')' | '}' | ']') || character == char::from(34)
            })
            .to_string();
        if !cleaned.is_empty() && !seen.contains(&cleaned) && is_trusted_auth_url(&cleaned) {
            queue.push_back(cleaned);
        }
    }
}

fn is_trusted_auth_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    matches!(
        url.host_str().unwrap_or_default(),
        "accounts.x.ai" | "auth.x.ai" | "auth.grok.com" | "auth.grokipedia.com"
    )
}

fn read_sso_cookie(jar: &Jar) -> Option<String> {
    for raw_url in [
        "https://accounts.x.ai/",
        "https://auth.x.ai/",
        "https://grok.com/",
        "https://auth.grok.com/",
    ] {
        let Ok(url) = Url::parse(raw_url) else {
            continue;
        };
        let Some(header) = jar.cookies(&url) else {
            continue;
        };
        let Ok(header) = header.to_str() else {
            continue;
        };
        for cookie in header.split(';') {
            let Some((name, value)) = cookie.trim().split_once('=') else {
                continue;
            };
            if matches!(name.trim(), "sso" | "sso-rw" | "sso_token") && !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

async fn post_registration_init(client: &Client, sso: &str, context: &GrokRegisterContext) {
    let tos_frame = vec![0, 0, 0, 0, 2, 0x10, 1];
    let tos_ok = client
        .post("https://accounts.x.ai/auth_mgmt.AuthManagement/SetTosAcceptedVersion")
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("x-user-agent", "connect-es/2.1.1")
        .header("origin", "https://accounts.x.ai")
        .header("referer", "https://accounts.x.ai/accept-tos")
        .body(tos_frame)
        .send()
        .await
        .is_ok_and(|response| response.status().is_success());

    let current_year = Utc::now().year();
    let year = current_year - rand::thread_rng().gen_range(25..=45);
    let month = rand::thread_rng().gen_range(1..=12);
    let day = rand::thread_rng().gen_range(1..=28);
    let birth_date = format!("{year:04}-{month:02}-{day:02}");
    let birth_ok = client
        .post("https://grok.com/rest/auth/set-birth-date")
        .header("content-type", "application/json")
        .header("origin", "https://grok.com")
        .header("referer", "https://grok.com/")
        .header("cookie", format!("sso={sso}; sso-rw={sso}"))
        .json(&json!({"birthDate": birth_date}))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success());

    if tos_ok && birth_ok {
        log(context, "success", "Grok 服务条款与生日初始化完成");
    } else {
        log(
            context,
            "warn",
            &format!("Grok 账号已注册，附加初始化未完全完成（tos={tos_ok}, birth={birth_ok}）"),
        );
    }
}

fn reject_blocked_response(status: u16, body: &str, action: &str) -> Result<(), String> {
    let lower = body.to_ascii_lowercase();
    let blocked = [
        "just a moment",
        "attention required",
        "sorry, you have been blocked",
        "cf-browser-verification",
        "verifying you are human",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if blocked || matches!(status, 401 | 429 | 503) || (status == 403 && lower.contains("<html")) {
        return Err(format!(
            "{action}被 Cloudflare/网络环境拦截（HTTP {status}），请配置与 Turnstile 一致的可用代理后重试"
        ));
    }
    if !(200..300).contains(&status) {
        return Err(format!("{action}失败（HTTP {status}）"));
    }
    Ok(())
}

async fn check_cancelled(dl: &Arc<DataLake>, run_id: &str) -> Result<(), String> {
    match dl.get_workflow_run_status(run_id).await {
        Ok(status) if status == "cancelled" => Err("cancelled".to_string()),
        _ => Ok(()),
    }
}

fn random_name() -> (&'static str, &'static str) {
    const FIRST_NAMES: &[&str] = &[
        "James",
        "Mary",
        "Robert",
        "Patricia",
        "John",
        "Jennifer",
        "Michael",
        "Linda",
        "David",
        "Elizabeth",
        "William",
        "Barbara",
        "Richard",
        "Susan",
        "Joseph",
        "Jessica",
    ];
    const LAST_NAMES: &[&str] = &[
        "Smith",
        "Johnson",
        "Williams",
        "Brown",
        "Jones",
        "Garcia",
        "Miller",
        "Davis",
        "Rodriguez",
        "Martinez",
        "Wilson",
        "Anderson",
        "Taylor",
        "Moore",
        "Jackson",
        "Lee",
    ];
    let mut rng = rand::thread_rng();
    (
        FIRST_NAMES[rng.gen_range(0..FIRST_NAMES.len())],
        LAST_NAMES[rng.gen_range(0..LAST_NAMES.len())],
    )
}

pub fn generate_password() -> String {
    let random: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(14)
        .map(char::from)
        .collect();
    format!("N{random}!a7#")
}

fn log(context: &GrokRegisterContext, level: &str, message: &str) {
    if let Some(callback) = context.step_callback.as_ref() {
        callback(level, message);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        chromium_proxy_config, find_action_id, find_site_key, grpc_message, is_trusted_auth_url,
        validate_solver_url,
    };

    #[test]
    fn prepares_authenticated_http_proxy_for_chromium() {
        let config = chromium_proxy_config(
            "http://user%40example.com:p%40ssword@proxy.example.com:8080/path",
        )
        .expect("proxy should be valid")
        .expect("proxy should be configured");

        assert_eq!(config.server, "http://proxy.example.com:8080");
        assert_eq!(
            config.credentials,
            Some(("user@example.com".to_string(), "p@ssword".to_string()))
        );
    }

    #[test]
    fn normalizes_proxy_scheme_and_rejects_authenticated_socks() {
        let config = chromium_proxy_config("socks5h://proxy.example.com:1080")
            .expect("proxy should be valid")
            .expect("proxy should be configured");
        assert_eq!(config.server, "socks5://proxy.example.com:1080");
        assert!(config.credentials.is_none());

        let error = match chromium_proxy_config("socks5://user:pass@proxy.example.com:1080") {
            Err(error) => error,
            Ok(_) => panic!("authenticated SOCKS proxy should be rejected"),
        };
        assert!(error.contains("SOCKS"));
    }

    #[test]
    fn extracts_dynamic_signup_parameters() {
        let action = format!("7f{}", "a".repeat(40));
        let source = format!(
            r#"window.cfg={{\"sitekey\":\"0x4AAAA-test_key\"}};createServerReference)(\"{action}\")"#
        );
        assert_eq!(find_site_key(&source).as_deref(), Some("0x4AAAA-test_key"));
        assert_eq!(find_action_id(&source).as_deref(), Some(action.as_str()));
    }

    #[test]
    fn frames_grpc_web_messages() {
        let framed = grpc_message(&["a@b.co", "ABC123"]);
        assert_eq!(framed[0], 0);
        assert_eq!(
            u32::from_be_bytes(framed[1..5].try_into().unwrap()) as usize,
            framed.len() - 5
        );
        assert_eq!(framed[5], 0x0a);
    }

    #[test]
    fn limits_auth_hops_to_known_https_hosts() {
        assert!(is_trusted_auth_url("https://auth.x.ai/set-cookie?q=token"));
        assert!(!is_trusted_auth_url(
            "https://auth.x.ai.evil.test/set-cookie?q=token"
        ));
        assert!(!is_trusted_auth_url("http://auth.x.ai/set-cookie?q=token"));
    }

    #[test]
    fn validates_solver_urls() {
        assert!(validate_solver_url("http://127.0.0.1:5072").is_ok());
        assert!(validate_solver_url("file:///tmp/solver").is_err());
        assert!(validate_solver_url("http://user:pass@localhost:5072").is_err());
    }
}
