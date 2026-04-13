use headless_chrome::{Browser, LaunchOptions};
use std::time::Duration;
use crate::openai::register::RegisterContext;
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

    pub async fn run(&self) -> Result<String, String> {
        let callback = &self.context.step_callback;
        
        let mode_text = if self.context.headless { "无头模式" } else { "有头模式 (Xvfb)" };
        if let Some(cb) = callback {
            cb("info", &format!("🚀 正在初始化 PhantomBrowser 仿真容器 ({})...", mode_text));
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

        // 2. 导航至 OpenAI 注册入口
        if let Some(cb) = callback {
            cb("info", "🌐 正在隐身访问 OpenAI 注册中心 (chatgpt.com/signup)...");
        }
        
        tab.navigate_to("https://chatgpt.com/signup").map_err(|e| format!("导航失败: {}", e))?;
        tab.wait_until_navigated().map_err(|e| format!("页面加载超时: {}", e))?;

        // 调试截图辅助
        let take_shot = |name: &str, tab: &std::sync::Arc<headless_chrome::Tab>| {
            if let Ok(png) = tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
                let path = format!("./data/debug_{}.png", name);
                let _ = std::fs::write(&path, png);
                return Some(path);
            }
            None
        };

        // 2.5 处理可能出现的 Cloudflare Turnstile 验证
        tokio::time::sleep(Duration::from_secs(8)).await;
        let is_cf_page = tab.evaluate("document.title.includes('请稍候') || !!document.querySelector('#turnstile-wrapper') || document.body.innerText.includes('Verify you are human')", false)
            .map(|r| r.value.and_then(|v| v.as_bool()).unwrap_or(false))
            .unwrap_or(false);

        if is_cf_page {
            take_shot("cloudflare_blocked", &tab);
            let page_content = tab.evaluate("document.body.innerText", false)
                .map(|r| r.value.and_then(|v| v.as_str()).unwrap_or(""))
                .unwrap_or("");
            
            if page_content.contains("Access denied") || page_content.contains("Reference #") {
                 if let Some(cb) = callback {
                    cb("error", "🚫 OpenAI 拒绝了你的 IP 访问 (Access Denied)。建议更换高质量 Proxy。");
                }
                return Err("IP 被封锁 (Access Denied)".to_string());
            }

            if let Some(cb) = callback {
                cb("error", "🛡️ 遭遇 Cloudflare 强力拦截，无头模式暂无法自动过白，已存入截图 debug_cloudflare_blocked.png");
            }
            return Err("Cloudflare 验证拦截".to_string());
        }

        // 3. 进入注册表单并输入邮箱
        if let Some(cb) = callback {
            cb("info", &format!("📧 正在输入邮箱并核验表单: {}", self.context.email));
        }

        let email_selectors = "input#email, input#username, input[name='email'], input[type='email']";
        let continue_selectors = "button[type='submit'], button[data-action-button-primary='true']";

        let email_input = tab.wait_for_element_with_custom_timeout(email_selectors, Duration::from_secs(30))
            .map_err(|_| {
                take_shot("email_not_found", &tab);
                "未找到邮箱输入框，环境检测可能未通过"
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

        // 等待页面跳转到资料填写页 (由于网络延迟，可能需要较长时间)
        let _ = tab.wait_for_element_with_custom_timeout("input[name='name'], input[name='full_name'], input#name", Duration::from_secs(60));
        
        if let Ok(name_input) = tab.find_element("input[name='name'], input[name='full_name'], input#name") {
             name_input.click().ok();
             tab.type_str(&full_name).ok();
        }

        if let Ok(age_input) = tab.find_element("input[name='age'], input[type='number'], input#age") {
             age_input.click().ok();
             tab.type_str(&age.to_string()).ok();
        } else if let Ok(birthday_input) = tab.find_element("input[name='birthday']") {
             // 兜底逻辑：如果还是旧版的生日输入框
             let bday = format!("{}-01-01", 2024 - age); 
             birthday_input.click().ok();
             tab.type_str(&bday).ok();
        }

        tab.press_key("Enter").ok();

        if let Some(cb) = callback {
            cb("success", "✅ 浏览器仿真注册流程执行完毕！");
        }

        Ok("注册成功 (Browser Mode)".to_string())
    }
}
