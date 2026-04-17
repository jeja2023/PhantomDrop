use headless_chrome::{Browser, LaunchOptions};
use std::time::Duration;
use crate::openai::register::{RegisterContext, build_client};
use crate::openai::sentinel;
use anyhow::Result;

/**
 * PhantomBrowser 驱动程序
 * 借鉴 SimpleAuthFlow 的插件逻辑，使用 CDP (Chrome DevTools Protocol) 
 * 实现绕过检测的自动化注册。
 */

pub struct BrowserDriver {
    pub context: RegisterContext,
}

impl BrowserDriver {
    pub fn new(context: RegisterContext) -> Self {
        Self { context }
    }

    pub async fn run(&self) -> Result<crate::openai::register::RegisterResult, String> {
        let callback = &self.context.step_callback;
        
        let mode_text = if self.context.headless { "无头模式" } else { "有头模式 (Xvfb)" };
        if let Some(cb) = callback {
            cb("info", &format!("🚀 正在初始化 PhantomBrowser 仿真容器 ({})...", mode_text));
        }

        // --- 核心增强：环境预检 (IP 检查) ---
        if let Some(cb) = callback {
            cb("info", "[EnvCheck] 正在探测浏览器出口 IP 环境...");
        }

        let pre_client = build_client(self.context.proxy_url.as_deref())?;
        match sentinel::check_ip_quality(&pre_client).await {
            Ok(info) => {
                if let Some(cb) = callback {
                    let msg = format!(
                        "环境探测成功 | IP: {} | 归属地: {} | 组织: {} | 风险评估: {}",
                        info.ip,
                        info.country,
                        info.org,
                        if info.is_datacenter { "⚠️ 机房/数据中心 (高风险)" } else { "✅ 住宅/基站 (低风险)" }
                    );
                    cb(if info.is_datacenter { "warn" } else { "success" }, &msg);
                }
            }
            Err(e) => {
                if let Some(cb) = callback {
                    cb("warn", &format!("环境预检跳过 (检测服务暂时不可达): {}", e));
                }
            }
        }

        // 1. 启动浏览器 (极致伪装以绕过检测)
        let mut launch_args = vec![
            "--disable-blink-features=AutomationControlled".to_string(),
            "--no-sandbox".to_string(),
            "--disable-dev-shm-usage".to_string(), 
            "--disable-infobars".to_string(),
            "--window-position=0,0".to_string(),
            "--ignore-certificate-errors".to_string(),
            "--disable-web-security".to_string(),
            "--allow-running-insecure-content".to_string(),
            "--disable-gpu".to_string(), // 虚拟显示环境建议禁用 GPU
            "--hide-scrollbars".to_string(),
            "--mute-audio".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-background-timer-throttling".to_string(),
            "--disable-backgrounding-occluded-windows".to_string(),
            "--disable-breakpad".to_string(),
            "--disable-client-side-phishing-detection".to_string(),
            "--disable-default-apps".to_string(),
            "--disable-extensions".to_string(),
            "--use-fake-ui-for-media-stream".to_string(),
            "--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36".to_string(),
        ];

        if let Some(ref proxy) = self.context.proxy_url {
            launch_args.push(format!("--proxy-server={}", proxy));
        }

        let options = LaunchOptions::default_builder()
            .headless(self.context.headless)  // 根据配置决定是否开启无头模式
            .window_size(Some((1920, 1080)))
            .idle_browser_timeout(Duration::from_secs(300))
            .args(launch_args.iter().map(|s| std::ffi::OsStr::new(s)).collect())
            .build()
            .map_err(|e| format!("浏览器启动失败: {}", e))?;

        let browser = Browser::new(options).map_err(|e| format!("无法连接到 Chrome 实例: {}", e))?;
        let tab = browser.new_tab().map_err(|e| format!("打开标签页失败: {}", e))?;

        // 注入增强型指纹伪装脚本 (极致风控过级)
        let stealth_script = r#"
            // 1. 隐藏 WebDriver
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });

            // 2. 伪造 WebGL 指纹 (使用更真实的显卡信息)
            const getParameter = WebGLRenderingContext.prototype.getParameter;
            WebGLRenderingContext.prototype.getParameter = function(parameter) {
                if (parameter === 37445) return 'Google Inc. (NVIDIA)';
                if (parameter === 37446) return 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)';
                return getParameter.apply(this, arguments);
            };

            // 3. 注入 Chrome Runtime 模拟 (无头模式通常缺失)
            window.chrome = { runtime: {} };

            // 4. 修复 Permissions 状态
            const originalQuery = window.navigator.permissions.query;
            window.navigator.permissions.query = (parameters) => (
                parameters.name === 'notifications' ?
                Promise.resolve({ state: Notification.permission }) :
                originalQuery(parameters)
            );

            // 5. 伪造 Plugins, Languages 和 Timezone
            Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
            Object.defineProperty(navigator, 'languages', { get: () => ['en-US', 'en'] });
            
            // 6. 随机化 Canvas 噪音 (轻微改动以混淆唯一的 Canvas ID)
            const originalToDataURL = HTMLCanvasElement.prototype.toDataURL;
            HTMLCanvasElement.prototype.toDataURL = function(type) {
                const res = originalToDataURL.apply(this, arguments);
                return res; // 目前仅做钩子留存，可根据需要加盐
            };

            // 7. 伪造硬件并发数和设备内存 (避免默认的 0 或极端值)
            Object.defineProperty(navigator, 'hardwareConcurrency', { get: () => 8 });
            Object.defineProperty(navigator, 'deviceMemory', { get: () => 8 });
        "#;

        let _ = tab.call_method(headless_chrome::protocol::cdp::Page::AddScriptToEvaluateOnNewDocument {
            source: stealth_script.to_string(),
            world_name: None,
            include_command_line_api: None,
            run_immediately: None,
        });

        // 2. 导航至 OpenAI 注册入口 (使用 screen_hint 强制跳转至注册页)
        if let Some(cb) = callback {
            cb("info", "🌐 正在隐身访问 OpenAI 注册中心 (chatgpt.com/signup)...");
        }
        
        tab.navigate_to("https://chatgpt.com/auth/login?screen_hint=signup").map_err(|e| format!("导航失败: {}", e))?;
        tab.wait_until_navigated().map_err(|e| format!("页面加载超时: {}", e))?;

        // 记录导航后的状态
        let current_url = tab.get_url();
        let page_title = tab.evaluate("document.title", false)
            .ok().and_then(|r| r.value.and_then(|v| v.as_str().map(|s| s.to_string())))
            .unwrap_or_else(|| "未知标题".to_string());
            
        if let Some(cb) = callback {
            cb("info", &format!("📍 页面已加载 | 标题: {} | URL: {}", page_title, current_url));
        }

        // 2.2 首页中转处理：如果跳转到了首页而非注册页，尝试点击“Sign up”按钮
        tokio::time::sleep(Duration::from_secs(4)).await;
        // 增加对中文落地页“开始使用”的识别
        let is_home_page = tab.evaluate("document.body.innerText.includes('Where should we begin?') || document.body.innerText.includes('开始使用') || (document.body.innerText.includes('ChatGPT') && document.querySelectorAll('button, a').length < 50)", false)
            .map(|r| r.value.and_then(|v| v.as_bool()).unwrap_or(false))
            .unwrap_or(false);

        if is_home_page {
            if let Some(cb) = callback {
                cb("info", "📍 识别到落地页中转，正在寻找注册入口...");
            }
            // 尝试多种可能的注册按钮选择器，增加“免费注册”文案支持
            let _ = tab.evaluate(&format!("(function(){{ 
                const keywords = ['Sign up', '免费注册', '注册', 'Get started', '开始使用', 'Sign Up'];
                const btn = Array.from(document.querySelectorAll('a, button')).find(el => 
                    keywords.some(k => el.innerText.includes(k)) || 
                    (el.getAttribute('href') && el.getAttribute('href').includes('signup'))
                );
                if(btn) {{
                    btn.click();
                }}
            }})()"), false);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        // 调试截图辅助
        let take_shot = |name: &str, tab: &std::sync::Arc<headless_chrome::Tab>| {
            let _ = std::fs::create_dir_all("./data");
            if let Ok(png) = tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
                let filename = format!("debug_{}.png", name);
                let path = format!("./data/{}", filename);
                let _ = std::fs::write(&path, png);
                if let Some(cb) = callback {
                    cb("warn", &format!("📸 自动抓拍已保存: [点击查看](/debug/{})", filename));
                }
                return Some(path);
            }
            None
        };

        // 2.5 处理可能出现的 Cloudflare Turnstile 验证
        let mut cf_retry = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let is_cf_page = tab.evaluate("document.title.includes('请稍候') || !!document.querySelector('#turnstile-wrapper') || document.body.innerText.includes('Verify you are human')", false)
                .map(|r| r.value.and_then(|v| v.as_bool()).unwrap_or(false))
                .unwrap_or(false);

            if !is_cf_page {
                break;
            }

            if cf_retry >= 5 {
                take_shot("cloudflare_blocked", &tab);
                 if let Some(cb) = callback {
                    cb("error", "🛡️ 遭遇 Cloudflare 持续拦截，已尝试点击但未能通过，建议检查 Proxy 质量。");
                }
                return Err("Cloudflare 验证拦截超时".to_string());
            }

            if let Some(cb) = callback {
                cb("warn", &format!("🛡️ 正在尝试通过 Cloudflare 验证 (第 {}/5 次尝试)...", cf_retry + 1));
            }

            // 1. 尝试将验证框滚动到视野中心
            let _ = tab.evaluate(r#"
                (function() {
                    const el = document.querySelector('#turnstile-wrapper') || document.querySelector('iframe[src*="cloudflare"]');
                    if (el) { el.scrollIntoView({block: "center"}); }
                })()
            "#, false);

            // 2. 模拟物理点击
            if let Ok(el) = tab.find_element("#turnstile-wrapper, iframe[src*='cloudflare']") {
                let _ = el.click();
            }

            cf_retry += 1;
            tokio::time::sleep(Duration::from_secs(8)).await;
        }

        // 3. 进入注册表单并输入邮箱
        if let Some(cb) = callback {
            cb("info", &format!("📧 正在输入邮箱并核验表单: {}", self.context.email));
        }

        let email_selectors = "input#email, input#username, input[name='email'], input[type='email']";
        let continue_selectors = "button[type='submit'], button[data-action-button-primary='true']";

        let email_input = tab.wait_for_element_with_custom_timeout(email_selectors, Duration::from_secs(30))
            .map_err(|_| {
                let current_url = tab.get_url();
                take_shot("email_not_found", &tab);
                format!("未找到邮箱输入框，环境检测可能未通过 (当前 URL: {})", current_url)
            })?;
        
        email_input.click().ok();
        tab.type_str(&self.context.email).map_err(|e| format!("邮箱输入失败: {}", e))?;
        take_shot("after_email_input", &tab);

        if let Ok(btn) = tab.find_element(continue_selectors) {
            btn.click().ok();
        } else {
            tab.press_key("Enter").ok();
        }

        tokio::time::sleep(Duration::from_secs(5)).await;

        // 4. 输入密码
        if let Some(cb) = callback {
            cb("info", "🔐 正在注入安全密码...");
        }
        
        let pwd_selectors = "input#password, input[name='password'], input[type='password']";
        let pwd_input_res = tab.wait_for_element_with_custom_timeout(pwd_selectors, Duration::from_secs(30));
        
        if pwd_input_res.is_err() {
            take_shot("password_not_found", &tab);
            return Err("进入密码设置页失败，可能邮箱已被黑名单或需邮箱验证".to_string());
        }

        let pwd_input = pwd_input_res.unwrap();
        pwd_input.click().ok();
        tab.type_str(&self.context.password).ok();
        take_shot("after_password_input", &tab);
        
        tab.press_key("Enter").ok();

        // 5. 等待验证邮件
        if let Some(cb) = callback {
            cb("warn", "📩 请在后台查看验证邮件并点击激活链接...");
        }
        
        // 此处逻辑可以参考协议模式中的 poll_otp_by_email
        // 插件模式中是自动跳转到邮箱页，这里我们也建议用户等待
        tokio::time::sleep(Duration::from_secs(5)).await;

        // 6. 个人资料填写 (姓名和生日)
        if let Some(cb) = callback {
            cb("info", "👤 正在同步个人资料 (姓名/生日)...");
        }
        
        // 提前生成随机值，避免 ThreadRng 在 await 期间被持有
        let (full_name, age) = {
            let mut rng = rand::thread_rng();
            use rand::Rng;
            let first_names = ["Oliver", "Jack", "Harry", "Jacob", "Charlie", "Thomas", "George", "Oscar", "James", "William", "Alice", "Emma", "Sophia", "Isabella", "Mia"];
            let last_names = ["Smith", "Jones", "Taylor", "Williams", "Brown", "Davies", "Evans", "Wilson", "Thomas", "Roberts", "Johnson", "Walker", "White", "Edwards", "Churchill"];

            let n = self.context.full_name.as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let f = first_names[rng.gen_range(0..first_names.len())];
                    let l = last_names[rng.gen_range(0..last_names.len())];
                    format!("{} {}", f, l)
                });
                
            let a = self.context.age.unwrap_or_else(|| rng.gen_range(19..45));
            (n, a)
        };

        if let Some(cb) = callback {
            cb("info", &format!("资料生成 -> 姓名: {}, 年龄: {}", full_name, age));
        }

        // 严格等待姓名输入框出现，若超时则认为注册失败 (账号可能被拦截或环境检测通过但未跳转)
        tab.wait_for_element_with_custom_timeout("input[name='name'], input[name='full_name'], input#name", Duration::from_secs(60))
            .map_err(|_| {
                take_shot("profile_timeout", &tab);
                "已完成注册表单提交，但无法进入个人资料填写页 (可能是 IP 质量较差导致被拦截)"
            })?;
        
        if let Ok(name_input) = tab.find_element("input[name='name'], input[name='full_name'], input#name") {
             name_input.click().ok();
             tab.type_str(&full_name).ok();
             take_shot("after_name_input", &tab);
        }

        if let Ok(age_input) = tab.find_element("input[name='age'], input[type='number'], input#age") {
             age_input.click().ok();
             tab.type_str(&age.to_string()).ok();
             take_shot("after_age_input", &tab);
        } else if let Ok(birthday_input) = tab.find_element("input[name='birthday']") {
             // 兜底逻辑：如果还是旧版的生日输入框
             let bday = format!("{}-01-01", 2024 - age); 
             birthday_input.click().ok();
             tab.type_str(&bday).ok();
        }

        // 优先尝试点击继续按钮，如果不成功则按下回车
        let _ = tab.evaluate(r#"(function(){ 
            const keywords = ['Continue', '继续', '确认', 'Next', '下一步'];
            const btn = Array.from(document.querySelectorAll('button')).find(el => 
                keywords.some(k => el.innerText.includes(k))
            );
            if(btn) { btn.click(); }
        })()"#, false);
        
        tokio::time::sleep(Duration::from_secs(1)).await;
        tab.press_key("Enter").ok();

        // 6.5 处理可能的后续确认弹窗或引导页 (关键：确保进入最终的聊天界面)
        tokio::time::sleep(Duration::from_secs(3)).await;
        let _ = tab.evaluate(r#"(function(){ 
            const keywords = ['Continue', '继续', '确认', 'Agree', '同意', 'Next', '下一步', 'Done', '完成', 'Okay'];
            const buttons = Array.from(document.querySelectorAll('button, [role="button"]'));
            for (const btn of buttons) {
                if (keywords.some(k => btn.innerText.includes(k))) {
                    try { btn.click(); } catch(e) {}
                }
            }
        })()"#, false);

        // 再次兜底等待，确保页面跳转至 chatgpt.com 首页
        if let Some(cb) = callback {
            cb("info", "⌛ 正在等待 Dashboard 界面加载 (可能需要绕过引导弹窗)...");
        }
        
        take_shot("waiting_for_dashboard", &tab);

        let mut dash_found = false;
        for i in 0..5 {
             // 检查是否出现了聊天输入框或侧边栏，这代表进入了主界面
            let is_dash = tab.evaluate("document.querySelector('#prompt-textarea, [data-testid=\"composer-input\"], nav') !== null", false)
                .map(|r| r.value.and_then(|v| v.as_bool()).unwrap_or(false))
                .unwrap_or(false);
            
            if is_dash {
                dash_found = true;
                take_shot("dashboard_detected", &tab);
                break;
            }

            // 再次尝试点击可能的引导按钮
            let _ = tab.evaluate("Array.from(document.querySelectorAll('button')).forEach(b => { if(['Next', 'Done', '继续', '完成', 'Okay'].some(k => b.innerText.includes(k))) b.click(); })", false);
            if i % 2 == 1 {
                take_shot(&format!("dashboard_waiting_step_{}", i), &tab);
            }
            tokio::time::sleep(Duration::from_secs(4)).await;
        }

        if dash_found {
             if let Some(cb) = callback { cb("success", "📍 已成功抵达 ChatGPT 主控台界面"); }
        } else {
             let final_url = tab.get_url();
             if let Some(cb) = callback { 
                 cb("warn", &format!("📍 未能识别到主控台特征 (当前 URL: {}), 正在尝试强行重定向并提取...", final_url)); 
             }
             take_shot("dashboard_not_detected", &tab);
             
             // 如果停留在了 auth0 或者错误的页面，强行跳转到首页
             if final_url.contains("auth0") || final_url.contains("signup") || final_url.contains("profile") {
                 let _ = tab.navigate_to("https://chatgpt.com/");
                 tokio::time::sleep(Duration::from_secs(8)).await;
             }
        }

        // 7. 提取 Access Token (关键步骤)
        if let Some(cb) = callback {
            cb("info", "🔑 正在等待会话就绪并提取 Access Token...");
        }

        // 增强型循环尝试提取，直到成功或超时 (2 分钟)
        let mut token_extracted = None;
        for i in 0..24 {
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 尝试执行 JS 获取 session，增加了对 localStorage 和备用路径的检查
            let js = r#"
                (async function() {
                    try {
                        // 1. 尝试 Fetch Next-Auth Session (由于此时已在 chatgpt.com，此方法最可靠)
                        const resp = await fetch('/api/auth/session');
                        if (resp.ok) {
                            const data = await resp.json();
                            if(data.accessToken) return data.accessToken;
                        }
                    } catch (e) {}
                    
                    try {
                        // 2. 遍历 LocalStorage 寻找 JWT 特征字符串
                        for (let j = 0; j < localStorage.length; j++) {
                            const key = localStorage.key(j);
                            const val = localStorage.getItem(key);
                            if (val && val.startsWith('eyJ') && val.split('.').length === 3) {
                                return val;
                            }
                        }
                    } catch (e) {}

                    try {
                        // 3. 遍历 SessionStorage 寻找 JWT 特征字符串
                        for (let j = 0; j < sessionStorage.length; j++) {
                            const key = sessionStorage.key(j);
                            const val = sessionStorage.getItem(key);
                            if (val && val.startsWith('eyJ') && val.split('.').length === 3) {
                                return val;
                            }
                        }
                    } catch (e) {}
                    
                    return null;
                })()
            "#;

            if let Ok(eval_res) = tab.evaluate(js, true) {
                if let Some(token) = eval_res.value.and_then(|v| v.as_str().map(|s| s.to_string())) {
                    if !token.is_empty() {
                        token_extracted = Some(token);
                        break;
                    }
                }
            }

            if i % 4 == 0 && i > 0 {
                let current_url = tab.get_url();
                if let Some(cb) = callback {
                    cb("info", &format!("尝试提取凭证中 (第 {}/24 次) [URL: {}]...", i + 1, current_url));
                }
                
                // 如果尝试多次依然失败且页面看起来是空白的，尝试刷新或重定向
                if i == 8 || i == 16 {
                    if current_url.contains("chatgpt.com") {
                        if let Some(cb) = callback { cb("warn", "🔄 提取停滞，尝试刷新页面以触发会话同步..."); }
                        let _ = tab.reload(false, None);
                    } else {
                        if let Some(cb) = callback { cb("warn", "🔄 页面偏移，正在尝试强制校准至 ChatGPT 首页..."); }
                        let _ = tab.navigate_to("https://chatgpt.com/");
                    }
                }
            }
        }

        // 尝试额外提取 Session Token (从 Cookie 中)
        // 尝试额外提取 Session Token (增加对不同命名的 Cookie 兼容)
        let mut session_extracted = None;
        if let Ok(cookies) = tab.get_cookies() {
            let session_cookie_names = [
                "__Secure-next-auth.session-token",
                "__Host-next-auth.session-token",
                "next-auth.session-token"
            ];
            for name in session_cookie_names {
                if let Some(cookie) = cookies.iter().find(|c| c.name == name) {
                    session_extracted = Some(cookie.value.clone());
                    break;
                }
            }
        }

        if let Some(cb) = callback {
            if token_extracted.is_some() || session_extracted.is_some() {
                cb("success", "✅ 浏览器仿真注册流程执行完毕，已获取访问凭证！");
                Ok(crate::openai::register::RegisterResult {
                    email: self.context.email.clone(),
                    password: self.context.password.clone(),
                    access_token: token_extracted,
                    refresh_token: None,
                    session_token: session_extracted, 
                    device_id: self.context.device_id.clone(),
                    workspace_id: Some("ws-browser-org".to_string()),
                })
            } else {
                cb("error", "❌ 注册流程可能已走完，但未能在规定时间内提取到任何有效 Token。请检查快照确认为何未进入主控台。");
                take_shot("extraction_failed_final", &tab);
                Err("凭证提取完全失败 (Access Token & Session Token 均为 None)".to_string())
            }
        } else {
            Err("Callback missing".to_string())
        }
    }
}
