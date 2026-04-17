/**
 * OpenAI 协议常量定义
 * 封装所有与 OpenAI 认证服务器交互所需的端点、Client ID 及 Scope 配置
 */

/// OpenAI 官方 OAuth Client ID
pub const OPENAI_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// OpenAI API 基地址

/// ChatGPT 前端身份认证基地址
/// Sentinel 令牌获取端点
pub const SENTINEL_ENDPOINT: &str = "https://sentinel.openai.com/backend-api/sentinel/req";

/// OAuth 授权端点
pub const AUTH_AUTHORIZE_URL: &str = "https://auth0.openai.com/authorize";

/// OAuth 令牌交换端点
pub const AUTH_TOKEN_URL: &str = "https://auth0.openai.com/oauth/token";

/// 注册提交端点
pub const AUTH_SIGNUP_URL: &str = "https://auth0.openai.com/u/signup";

/// 密码提交端点
pub const AUTH_PASSWORD_URL: &str = "https://auth0.openai.com/u/signup/password";

/// 提交 OTP 验证端点
pub const AUTH_OTP_VALIDATE_URL: &str = "https://chatgpt.com/backend-api/accounts/email-otp/validate";

/// 请求手机号验证码端点
pub const AUTH_SMS_OTP_REQUEST_URL: &str = "https://chatgpt.com/backend-api/accounts/sms-otp/request";

/// 提交手机号验证码端点
pub const AUTH_SMS_OTP_VALIDATE_URL: &str = "https://chatgpt.com/backend-api/accounts/sms-otp/validate";

/// 创建用户信息端点
pub const AUTH_CREATE_USER_URL: &str = "https://chatgpt.com/backend-api/accounts/user/register";

/// OAuth Scope
pub const OPENAI_SCOPE: &str = "openid profile email offline_access";

/// OAuth 回调重定向 URI
pub const REDIRECT_URI: &str = "https://chatgpt.com/api/auth/callback/login-web";

/// 默认 User-Agent (匹配 Chrome 124 指纹)
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
