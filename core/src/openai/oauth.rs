/**
 * OpenAI OAuth/PKCE 工具模块
 * 实现 PKCE (Proof Key for Code Exchange) 的 S256 模式，
 * 用于 OAuth 2.0 授权码流程中的安全增强
 */
use rand::Rng;

/// PKCE 参数对
pub struct PkceParams {
    pub code_verifier: String,
    pub code_challenge: String,
}

/// 生成 PKCE code_verifier（43-128 字符的随机 URL-safe 字符串）
fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(43..=128);
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

/// 根据 code_verifier 生成 S256 code_challenge
/// code_challenge = BASE64URL(SHA256(code_verifier))
fn generate_code_challenge(verifier: &str) -> String {
    let hash = simple_sha256(verifier.as_bytes());
    base64url_encode(&hash)
}

/// 生成完整的 PKCE 参数对
pub fn generate_pkce() -> PkceParams {
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    PkceParams {
        code_verifier,
        code_challenge,
    }
}

/// 生成随机状态参数（防 CSRF）
pub fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

/// 生成设备指纹 ID（UUID v4 格式）
pub fn generate_device_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct CodexAuthData {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

impl CodexAuthData {
    pub fn get_email(&self) -> Option<String> {
        let parts: Vec<&str> = self.id_token.split('.').collect();
        if parts.len() >= 2 {
            use base64::Engine;
            if let Ok(decoded) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                    return json
                        .get("email")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
        }
        None
    }
}

/// 将回调 URL 转换为授权码并进行令牌交换
pub async fn exchange_codex_code(
    callback_url: &str,
    code_verifier: &str,
) -> Result<CodexAuthData, String> {
    let url = url::Url::parse(callback_url).map_err(|e| format!("URL 解析失败: {}", e))?;
    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or("回调 URL 中未找到授权码 (code)")?;

    let client = reqwest::Client::new();
    let params = [
        ("client_id", crate::openai::constants::OPENAI_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", "http://localhost:1455/auth/callback"),
        ("code_verifier", code_verifier),
    ];

    let response = client
        .post(crate::openai::constants::AUTH_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("令牌交换请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("令牌交换失败 ({}): {}", status, error_text));
    }

    let auth_data = response
        .json::<CodexAuthData>()
        .await
        .map_err(|e| format!("解析令牌响应失败: {}", e))?;

    Ok(auth_data)
}

// --- 辅助函数：Base64 URL 编码（无 padding） ---

fn base64url_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::with_capacity((data.len() * 4 + 2) / 3);
    let chunks = data.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        result.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        result.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(TABLE[(n & 0x3f) as usize] as char);
        }
    }
    result
}

// --- 辅助函数：简易 SHA-256 实现 ---

/// 供其他模块调用的 SHA-256 公开入口
pub fn simple_sha256_public(data: &[u8]) -> [u8; 32] {
    simple_sha256(data)
}

fn simple_sha256(data: &[u8]) -> [u8; 32] {
    // SHA-256 初始哈希值
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // SHA-256 轮常量
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    // 消息预处理：填充
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // 逐块处理
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = [0u8; 32];
    for i in 0..8 {
        let bytes = h[i].to_be_bytes();
        result[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }
    result
}

// --- 辅助函数：JWT 无感解析与 Mock 生成 ---

/// 解析 JWT 令牌的 Payload 负载数据（不进行签名验签，无感解析）
pub fn parse_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() >= 2 {
        use base64::Engine;
        // 支持 URL 安全与标准 Base64 编码（无 Padding 填充）
        let decoders = [
            base64::engine::general_purpose::URL_SAFE_NO_PAD,
            base64::engine::general_purpose::STANDARD_NO_PAD,
        ];
        for engine in decoders {
            if let Ok(decoded) = engine.decode(parts[1]) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                    return Some(json);
                }
            }
        }
        // 兜底尝试带 Padding 的 Base64 解码
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(parts[1]) {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                return Some(json);
            }
        }
        if let Ok(decoded) = base64::engine::general_purpose::URL_SAFE.decode(parts[1]) {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                return Some(json);
            }
        }
    }
    None
}

/// 提取出的账号 Auth 凭证核心元数据
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedAuthInfo {
    pub email: Option<String>,
    pub chatgpt_account_id: Option<String>,
    pub chatgpt_user_id: Option<String>,
    pub organization_id: Option<String>,
    pub plan_type: Option<String>,
}

pub const DEFAULT_OAUTH_EXPIRES_IN: i64 = 864000;
pub const DEFAULT_OAUTH_TOKEN_VERSION: i64 = 1778215057457;

pub struct OAuthCredentialInput<'a> {
    pub email: &'a str,
    pub access_token: Option<&'a str>,
    pub refresh_token: Option<&'a str>,
    pub id_token: Option<&'a str>,
    #[allow(dead_code)]
    pub workspace_id: Option<&'a str>,
    pub chatgpt_account_id: Option<&'a str>,
    pub chatgpt_user_id: Option<&'a str>,
    pub organization_id: Option<&'a str>,
    pub plan_type: Option<&'a str>,
    pub expires_in: Option<i64>,
    pub token_version: Option<i64>,
    pub stored_credentials: Option<&'a serde_json::Value>,
}

pub struct BuiltOAuthCredentials {
    pub json: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub chatgpt_account_id: String,
    pub chatgpt_user_id: String,
    pub organization_id: String,
    pub plan_type: String,
    pub expires_in: i64,
    pub token_version: i64,
}

pub fn parse_non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(parse_non_empty)
}

fn stored_string(stored: Option<&serde_json::Value>, key: &str) -> Option<String> {
    stored
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn stored_i64(stored: Option<&serde_json::Value>, key: &str) -> Option<i64> {
    stored.and_then(|value| value.get(key)).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse::<i64>().ok())
    })
}

fn looks_like_jwt(token: &str) -> bool {
    token.split('.').count() == 3 && parse_jwt_payload(token).is_some()
}

fn is_generated_token(token: &str) -> bool {
    token.contains(".fallback_signature")
        || token.contains(".mock_signature")
        || token.contains("compat_signature_for_cpa_parsing_only")
}

fn valid_access_token(value: Option<String>) -> Option<String> {
    value.filter(|token| looks_like_jwt(token) && !is_generated_token(token))
}

fn valid_refresh_token(value: Option<String>) -> Option<String> {
    value.filter(|token| token.starts_with("rt_"))
}

fn valid_id_token(value: Option<String>) -> Option<String> {
    value.filter(|token| looks_like_jwt(token) && !is_generated_token(token))
}

pub fn build_oauth_credentials_value(input: OAuthCredentialInput<'_>) -> serde_json::Value {
    let stored = input.stored_credentials;
    let email = non_empty(Some(input.email))
        .map(ToOwned::to_owned)
        .or_else(|| stored_string(stored, "email"))
        .unwrap_or_default();
    let access_token = valid_access_token(non_empty(input.access_token).map(ToOwned::to_owned))
        .or_else(|| valid_access_token(stored_string(stored, "access_token")))
        .unwrap_or_default();
    let refresh_token = valid_refresh_token(non_empty(input.refresh_token).map(ToOwned::to_owned))
        .or_else(|| valid_refresh_token(stored_string(stored, "refresh_token")))
        .unwrap_or_default();
    let id_token = valid_id_token(non_empty(input.id_token).map(ToOwned::to_owned))
        .or_else(|| valid_id_token(stored_string(stored, "id_token")))
        .unwrap_or_default();
    let parsed_auth = if !id_token.is_empty() {
        extract_auth_info_from_jwt(&id_token)
    } else {
        extract_auth_info_from_jwt(&access_token)
    };
    let has_real_oauth_token = !access_token.is_empty() || !id_token.is_empty();

    let chatgpt_account_id = parsed_auth
        .chatgpt_account_id
        .or_else(|| {
            has_real_oauth_token
                .then(|| non_empty(input.chatgpt_account_id).map(ToOwned::to_owned))
                .flatten()
        })
        .or_else(|| {
            has_real_oauth_token
                .then(|| stored_string(stored, "chatgpt_account_id"))
                .flatten()
        })
        .unwrap_or_default();
    let chatgpt_user_id = parsed_auth
        .chatgpt_user_id
        .or_else(|| {
            has_real_oauth_token
                .then(|| non_empty(input.chatgpt_user_id).map(ToOwned::to_owned))
                .flatten()
        })
        .or_else(|| {
            has_real_oauth_token
                .then(|| stored_string(stored, "chatgpt_user_id"))
                .flatten()
        })
        .unwrap_or_default();
    let organization_id = parsed_auth
        .organization_id
        .or_else(|| {
            has_real_oauth_token
                .then(|| non_empty(input.organization_id).map(ToOwned::to_owned))
                .flatten()
        })
        .or_else(|| {
            has_real_oauth_token
                .then(|| stored_string(stored, "organization_id"))
                .flatten()
        })
        .unwrap_or_default();
    let plan_type = parsed_auth
        .plan_type
        .or_else(|| {
            has_real_oauth_token
                .then(|| non_empty(input.plan_type).map(ToOwned::to_owned))
                .flatten()
        })
        .or_else(|| {
            has_real_oauth_token
                .then(|| stored_string(stored, "plan_type"))
                .flatten()
        })
        .unwrap_or_else(|| "free".to_string());
    let expires_in = input
        .expires_in
        .or_else(|| stored_i64(stored, "expires_in"))
        .unwrap_or(DEFAULT_OAUTH_EXPIRES_IN);
    let token_version = input
        .token_version
        .or_else(|| stored_i64(stored, "_token_version"))
        .or_else(|| stored_i64(stored, "token_version"))
        .unwrap_or(DEFAULT_OAUTH_TOKEN_VERSION);

    serde_json::json!({
        "_token_version": token_version,
        "access_token": access_token,
        "chatgpt_account_id": chatgpt_account_id,
        "chatgpt_user_id": chatgpt_user_id,
        "email": email,
        "expires_in": expires_in,
        "id_token": id_token,
        "organization_id": organization_id,
        "plan_type": plan_type,
        "refresh_token": refresh_token
    })
}

fn credential_string(credentials: &serde_json::Value, key: &str) -> String {
    credentials
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn credential_i64(credentials: &serde_json::Value, key: &str) -> i64 {
    credentials
        .get(key)
        .and_then(|value| value.as_i64())
        .unwrap_or_default()
}

pub fn build_oauth_credentials(input: OAuthCredentialInput<'_>) -> BuiltOAuthCredentials {
    let value = build_oauth_credentials_value(input);
    let json = {
        let has_complete_token_set =
            ["access_token", "refresh_token", "id_token"]
                .iter()
                .all(|key| {
                    value
                        .get(key)
                        .and_then(|value| value.as_str())
                        .is_some_and(|value| !value.trim().is_empty())
                });
        has_complete_token_set.then(|| value.to_string())
    };

    BuiltOAuthCredentials {
        access_token: credential_string(&value, "access_token"),
        refresh_token: credential_string(&value, "refresh_token"),
        id_token: credential_string(&value, "id_token"),
        chatgpt_account_id: credential_string(&value, "chatgpt_account_id"),
        chatgpt_user_id: credential_string(&value, "chatgpt_user_id"),
        organization_id: credential_string(&value, "organization_id"),
        plan_type: credential_string(&value, "plan_type"),
        expires_in: credential_i64(&value, "expires_in"),
        token_version: credential_i64(&value, "_token_version"),
        json,
    }
}

/// 从 ID Token 的 JWT Payload 中解析并提取完整的 OpenAI 账号多维元数据
pub fn extract_auth_info_from_jwt(id_token: &str) -> ExtractedAuthInfo {
    let mut info = ExtractedAuthInfo {
        email: None,
        chatgpt_account_id: None,
        chatgpt_user_id: None,
        organization_id: None,
        plan_type: None,
    };

    if let Some(payload) = parse_jwt_payload(id_token) {
        if let Some(email) = payload.get("email").and_then(|v| v.as_str()) {
            info.email = Some(email.to_string());
        }

        // 尝试从 OpenAI 专有的 Auth 命名空间 https://api.openai.com/auth 下提取
        if let Some(auth_claim) = payload.get("https://api.openai.com/auth") {
            if let Some(acct_id) = auth_claim
                .get("chatgpt_account_id")
                .and_then(|v| v.as_str())
            {
                info.chatgpt_account_id = Some(acct_id.to_string());
            }
            if let Some(user_id) = auth_claim.get("chatgpt_user_id").and_then(|v| v.as_str()) {
                info.chatgpt_user_id = Some(user_id.to_string());
            } else if let Some(user_id) = auth_claim.get("user_id").and_then(|v| v.as_str()) {
                info.chatgpt_user_id = Some(user_id.to_string());
            }
            if let Some(plan) = auth_claim.get("chatgpt_plan_type").and_then(|v| v.as_str()) {
                info.plan_type = Some(plan.to_string());
            } else if let Some(plan) = auth_claim.get("plan_type").and_then(|v| v.as_str()) {
                info.plan_type = Some(plan.to_string());
            }

            // 从默认组织（is_default: true）或者首个组织中提取组织 ID
            if let Some(orgs) = auth_claim.get("organizations").and_then(|v| v.as_array()) {
                let org_id = orgs
                    .iter()
                    .find(|o| {
                        o.get("is_default")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    })
                    .or_else(|| orgs.first())
                    .and_then(|o| o.get("id").and_then(|v| v.as_str()));
                if let Some(id) = org_id {
                    info.organization_id = Some(id.to_string());
                }
            }
        }

        // 顶层备用字段提取
        if info.chatgpt_account_id.is_none() {
            if let Some(acct_id) = payload.get("chatgpt_account_id").and_then(|v| v.as_str()) {
                info.chatgpt_account_id = Some(acct_id.to_string());
            }
        }
        if info.chatgpt_user_id.is_none() {
            if let Some(user_id) = payload.get("chatgpt_user_id").and_then(|v| v.as_str()) {
                info.chatgpt_user_id = Some(user_id.to_string());
            } else if let Some(sub) = payload.get("sub").and_then(|v| v.as_str()) {
                info.chatgpt_user_id = Some(sub.to_string());
            }
        }
        if info.plan_type.is_none() {
            if let Some(plan) = payload.get("plan_type").and_then(|v| v.as_str()) {
                info.plan_type = Some(plan.to_string());
            }
        }
        if info.organization_id.is_none() {
            if let Some(org_id) = payload.get("org_id").and_then(|v| v.as_str()) {
                info.organization_id = Some(org_id.to_string());
            } else if let Some(org_id) = payload.get("organization_id").and_then(|v| v.as_str()) {
                info.organization_id = Some(org_id.to_string());
            }
        }
    }

    info
}

/// 仿真生成包含完整 OpenAI 格式声称（Claims）的 Mock ID Token
pub fn generate_mock_id_token(email: &str) -> String {
    let chatgpt_account_id = uuid::Uuid::new_v4().to_string();
    let chatgpt_user_id = format!("user-{}", uuid::Uuid::new_v4().simple());
    let organization_id = format!("org-{}", uuid::Uuid::new_v4().simple());

    let payload = serde_json::json!({
        "iss": "https://auth.openai.com",
        "sub": format!("auth0|{}", uuid::Uuid::new_v4().simple()),
        "aud": [crate::openai::constants::OPENAI_CLIENT_ID],
        "exp": chrono::Utc::now().timestamp() + 86400,
        "iat": chrono::Utc::now().timestamp(),
        "email": email,
        "email_verified": true,
        "name": "Mary Johnson",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": chatgpt_account_id,
            "chatgpt_user_id": chatgpt_user_id,
            "chatgpt_plan_type": "free",
            "organizations": [
                {
                    "id": organization_id,
                    "is_default": true,
                    "role": "owner",
                    "title": "Personal"
                }
            ],
            "user_id": chatgpt_user_id
        }
    });

    let payload_str = serde_json::to_string(&payload).unwrap();
    use base64::Engine;
    let payload_b64 =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_str.as_bytes());

    format!(
        "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.{}.mock_signature",
        payload_b64
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_generates_valid_params() {
        let params = generate_pkce();
        assert!(params.code_verifier.len() >= 43);
        assert!(params.code_verifier.len() <= 128);
        assert!(!params.code_challenge.is_empty());
        // code_challenge 不应包含 = + / 等传统 Base64 字符
        assert!(!params.code_challenge.contains('='));
        assert!(!params.code_challenge.contains('+'));
        assert!(!params.code_challenge.contains('/'));
    }

    #[test]
    fn state_is_correct_length() {
        let state = generate_state();
        assert_eq!(state.len(), 32);
    }

    #[test]
    fn sha256_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = simple_sha256(b"");
        assert_eq!(hash[0], 0xe3);
        assert_eq!(hash[1], 0xb0);
        assert_eq!(hash[31], 0x55);
    }

    #[test]
    fn extracts_openai_auth_claims_from_mock_id_token() {
        let token = generate_mock_id_token("alice@example.com");
        let info = extract_auth_info_from_jwt(&token);

        assert_eq!(info.email.as_deref(), Some("alice@example.com"));
        assert!(
            info.chatgpt_account_id
                .as_deref()
                .is_some_and(|v| !v.is_empty())
        );
        assert!(
            info.chatgpt_user_id
                .as_deref()
                .is_some_and(|v| !v.is_empty())
        );
        assert!(
            info.organization_id
                .as_deref()
                .is_some_and(|v| !v.is_empty())
        );
        assert_eq!(info.plan_type.as_deref(), Some("free"));
    }

    #[test]
    fn oauth_credentials_do_not_fabricate_missing_tokens() {
        let first = build_oauth_credentials(OAuthCredentialInput {
            email: "bob@example.com",
            access_token: None,
            refresh_token: None,
            id_token: None,
            workspace_id: None,
            chatgpt_account_id: None,
            chatgpt_user_id: None,
            organization_id: None,
            plan_type: None,
            expires_in: None,
            token_version: None,
            stored_credentials: None,
        });
        let second = build_oauth_credentials(OAuthCredentialInput {
            email: "bob@example.com",
            access_token: None,
            refresh_token: None,
            id_token: None,
            workspace_id: None,
            chatgpt_account_id: None,
            chatgpt_user_id: None,
            organization_id: None,
            plan_type: None,
            expires_in: None,
            token_version: None,
            stored_credentials: None,
        });

        assert_eq!(first.id_token, second.id_token);
        assert_eq!(first.chatgpt_account_id, second.chatgpt_account_id);
        assert_eq!(first.chatgpt_user_id, second.chatgpt_user_id);
        assert_eq!(first.organization_id, second.organization_id);
        assert_eq!(first.id_token, "");
        assert_eq!(first.chatgpt_account_id, "");
        assert_eq!(first.chatgpt_user_id, "");
        assert_eq!(first.organization_id, "");
        assert_eq!(first.plan_type, "free");
        assert_eq!(first.expires_in, DEFAULT_OAUTH_EXPIRES_IN);
        assert_eq!(first.token_version, DEFAULT_OAUTH_TOKEN_VERSION);
        assert!(first.json.is_none());
    }

    #[test]
    fn oauth_credentials_merge_stored_values_without_losing_new_tokens() {
        let id_token = generate_realistic_test_id_token("carol@example.com");
        let stored = serde_json::json!({
            "chatgpt_account_id": "acct-existing",
            "chatgpt_user_id": "user-existing",
            "organization_id": "org-existing",
            "plan_type": "plus",
            "expires_in": "123",
            "_token_version": 456
        });
        let built = build_oauth_credentials(OAuthCredentialInput {
            email: "carol@example.com",
            access_token: Some(
                "eyJhbGciOiJSUzI1NiJ9.eyJhdWQiOlsiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS92MSJdfQ.signature",
            ),
            refresh_token: Some("rt_new-refresh"),
            id_token: Some(&id_token),
            workspace_id: None,
            chatgpt_account_id: None,
            chatgpt_user_id: None,
            organization_id: None,
            plan_type: None,
            expires_in: None,
            token_version: None,
            stored_credentials: Some(&stored),
        });

        assert_eq!(built.chatgpt_account_id, "acct-test");
        assert_eq!(built.chatgpt_user_id, "user-test");
        assert_eq!(built.organization_id, "org-test");
        assert_eq!(built.plan_type, "free");
        assert_eq!(built.expires_in, 123);
        assert_eq!(built.token_version, 456);
        assert!(
            built
                .json
                .as_deref()
                .is_some_and(|v| v.contains("rt_new-refresh"))
        );
    }

    fn generate_realistic_test_id_token(email: &str) -> String {
        let payload = serde_json::json!({
            "iss": "https://auth.openai.com",
            "sub": "auth0|test",
            "aud": [crate::openai::constants::OPENAI_CLIENT_ID],
            "exp": chrono::Utc::now().timestamp() + 86400,
            "iat": chrono::Utc::now().timestamp(),
            "email": email,
            "email_verified": true,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-test",
                "chatgpt_user_id": "user-test",
                "chatgpt_plan_type": "free",
                "organizations": [
                    {
                        "id": "org-test",
                        "is_default": true,
                        "role": "owner",
                        "title": "Personal"
                    }
                ],
                "user_id": "user-test"
            }
        });

        use base64::Engine;
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_string(&payload).unwrap().as_bytes());
        format!(
            "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.{}.signature",
            payload_b64
        )
    }
}
