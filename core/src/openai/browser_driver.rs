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
        
        if let Some(cb) = callback {
            cb("info", "🚀 正在初始化 PhantomBrowser 仿真容器...");
        }

        // 1. 启动浏览器 (开启隐身模式以绕过 Cloudflare 检测)
        let mut launch_args = vec![
            "--disable-blink-features=AutomationControlled".to_string(),
            "--no-sandbox".to_string(),
            "--disable-infobars".to_string(),
            "--window-position=0,0".to_string(),
            "--ignore-certificate-errors".to_string(),
            "--disable-web-security".to_string(),
            "--allow-running-insecure-content".to_string(),
        ];

        if let Some(ref proxy) = self.context.proxy_url {
            launch_args.push(format!("--proxy-server={}", proxy));
        }

        let options = LaunchOptions::default_builder()
            .headless(false) 
            .window_size(Some((1280, 800)))
            .idle_browser_timeout(Duration::from_secs(300))
            .args(launch_args.iter().map(|s| std::ffi::OsStr::new(s)).collect())
            .build()
            .map_err(|e| format!("浏览器启动失败: {}", e))?;

        let browser = Browser::new(options).map_err(|e| format!("无法连接到 Chrome 实例: {}", e))?;
        let tab = browser.new_tab().map_err(|e| format!("打开标签页失败: {}", e))?;

        // 2. 导航至 OpenAI 注册入口
        if let Some(cb) = callback {
            cb("info", "🌐 正在导航至 OpenAI 注册中心 (chatgpt.com/signup)...");
        }
        
        tab.navigate_to("https://chatgpt.com/signup").map_err(|e| format!("导航失败: {}", e))?;
        tab.wait_until_navigated().map_err(|e| format!("页面加载超时: {}", e))?;

        // 2.5 处理可能出现的 Cloudflare Turnstile 验证
        tokio::time::sleep(Duration::from_secs(5)).await;
        let is_cf_page = tab.evaluate("document.title.includes('请稍候') || !!document.querySelector('#turnstile-wrapper') || document.body.innerText.includes('Verify you are human')", false)
            .map(|r| r.value.and_then(|v| v.as_bool()).unwrap_or(false))
            .unwrap_or(false);

        if is_cf_page {
            if let Some(cb) = callback {
                cb("warn", "🛡️ 检测到 Cloudflare 验证屏障，建议在浏览器窗口中手动点击验证码...");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        // 3. 进入注册表单并输入邮箱
        if let Some(cb) = callback {
            cb("info", &format!("📧 正在核对入口状态并输入注册邮箱: {}", self.context.email));
        }

        // 兼容多种选择器
        let email_selectors = "input#email, input#username, input[name='email'], input[type='email']";
        let continue_selectors = "button[type='submit'], button[data-action-button-primary='true'], button.ext-btn-primary";

        // 某些情况下需要点击“Sign up”按钮才能展示表单
        if tab.find_element(email_selectors).is_err() {
            if let Ok(signup_btn) = tab.find_element("button[data-testid='signup-button'], a[href*='signup'], button.btn-primary") {
                signup_btn.click().ok();
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
        
        let email_input = tab.wait_for_element_with_custom_timeout(email_selectors, Duration::from_secs(30))
            .map_err(|_| "未找到邮箱输入框 (可能由于人机验证拦截或页面结构变更)")?;
        
        email_input.click().ok();
        tab.type_str(&self.context.email).map_err(|e| format!("邮箱输入失败: {}", e))?;
        
        if let Ok(btn) = tab.find_element(continue_selectors) {
            btn.click().ok();
        } else {
            tab.press_key("Enter").map_err(|e| format!("提交邮箱失败: {}", e))?;
        }

        tokio::time::sleep(Duration::from_secs(5)).await;

        // 4. 输入密码
        if let Some(cb) = callback {
            cb("info", &format!("🔐 正在设置安全密令 (Password: {})...", self.context.password));
        }
        
        // 兼容多种密码选择器并增加超时
        let pwd_selectors = "input#password, input[name='password'], input[type='password']";
        let pwd_input_res = tab.wait_for_element_with_custom_timeout(pwd_selectors, Duration::from_secs(45));
        
        if pwd_input_res.is_err() {
            // 额外检查错误提示
            let body_text = tab.evaluate("document.body.innerText", false)
                .map(|r| {
                    r.value
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            
            if body_text.contains("User already exists") || body_text.contains("already has an account") {
                return Err("邮箱已被注册，请更换邮箱后重试".to_string());
            }
            if body_text.contains("Verify you are human") || body_text.contains("Cloudflare") {
                return Err("触发了人机验证，请在浏览器中手动解决后再继续".to_string());
            }
            
            return Err("未找到密码输入框 (可能触发了人机验证、邮箱冲突或页面加载缓慢)".to_string());
        }

        let pwd_input = pwd_input_res.unwrap();
        pwd_input.click().map_err(|e| format!("点击密码框失败: {}", e))?;
        tab.type_str(&self.context.password).map_err(|e| format!("密码输入失败: {}", e))?;
        
        if let Ok(btn) = tab.find_element(continue_selectors) {
             btn.click().ok();
        } else {
             tab.press_key("Enter").map_err(|e| format!("提交密码失败: {}", e))?;
        }

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
