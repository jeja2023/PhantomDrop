use std::time::Duration;
use tokio::time::sleep;


pub struct SmsActivateClient {
    api_key: String,
    client: reqwest::Client,
}

impl SmsActivateClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// 获取号码，返回 (id, number)
    pub async fn get_number(&self, service: &str, country: Option<&str>) -> Result<(String, String), String> {
        let country = country.unwrap_or("0");
        let url = format!(
            "https://sms-activate.org/stubs/handler_api.php?api_key={}&action=getNumber&service={}&country={}",
            self.api_key, service, country
        );

        let response = self.client.get(&url).send().await.map_err(|e| e.to_string())?;
        let text = response.text().await.map_err(|e| e.to_string())?;

        if text.starts_with("ACCESS_NUMBER") {
            let parts: Vec<&str> = text.split(':').collect();
            if parts.len() >= 3 {
                return Ok((parts[1].to_string(), parts[2].to_string()));
            }
        }

        Err(format!("获取号码失败: {}", text))
    }

    /// 等待验证码，超时返回错误
    pub async fn wait_for_code(&self, id: &str, timeout_secs: u64) -> Result<String, String> {
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < timeout_secs {
            let url = format!(
                "https://sms-activate.org/stubs/handler_api.php?api_key={}&action=getStatus&id={}",
                self.api_key, id
            );

            let response = self.client.get(&url).send().await.map_err(|e| e.to_string())?;
            let text = response.text().await.map_err(|e| e.to_string())?;

            if text.starts_with("STATUS_OK") {
                let parts: Vec<&str> = text.split(':').collect();
                if parts.len() >= 2 {
                    return Ok(parts[1].to_string());
                }
            } else if text == "STATUS_WAIT_CODE" {
                // 继续等待
            } else if text == "STATUS_CANCEL" {
                return Err("号码已被取消".to_string());
            } else {
                return Err(format!("异常状态: {}", text));
            }

            sleep(Duration::from_secs(5)).await;
        }

        Err("等待验证码超时".to_string())
    }

    /// 标记完成或重发
    pub async fn set_status(&self, id: &str, status: &str) -> Result<(), String> {
        // status: 1 (重发), 3 (完成), 8 (取消)
        let url = format!(
            "https://sms-activate.org/stubs/handler_api.php?api_key={}&action=setStatus&id={}&status={}",
            self.api_key, id, status
        );

        let response = self.client.get(&url).send().await.map_err(|e| e.to_string())?;
        let _ = response.text().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
