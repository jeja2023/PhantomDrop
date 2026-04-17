

/// 以 JSON 格式推送账号 (适用于 Sub2API, NewAPI 等)
/// 以 JSON 格式推送账号 (适用于 Sub2API, NewAPI 等)
pub async fn upload_account_json(
    client: &reqwest::Client,
    api_url: &str,
    api_key: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    let res = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("x-api-key", api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("JSON 分发失败: {}", e))?;

    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("目标服务拒绝: {}", res.status()))
    }
}

/// 以 Multipart 格式推送账号 (适用于标准 CPA 协议平台)
pub async fn upload_account_multipart(
    client: &reqwest::Client,
    cpa_url: &str,
    cpa_key: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    let mut form = reqwest::multipart::Form::new();
    
    // 将 JSON 扁平化为 Form Data
    if let Some(obj) = payload.as_object() {
        for (k, v) in obj {
            let val_str = match v {
                serde_json::Value::String(s) => s.clone(),
                _ => v.to_string(),
            };
            form = form.text(k.clone(), val_str);
        }
    }

    let res = client
        .post(cpa_url)
        .header("Authorization", format!("Bearer {}", cpa_key))
        .header("x-api-key", cpa_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Multipart 分发失败: {}", e))?;

    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("CPA 平台拒绝: {}", res.status()))
    }
}
