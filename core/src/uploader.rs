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

/// 以 Multipart 格式推送账号 (兼容 CLIProxyAPI/Codex 协议平台)
pub async fn upload_account_multipart(
    client: &reqwest::Client,
    cpa_url: &str,
    cpa_key: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    // 准备文件名，通常使用账号地址作为标识
    let filename = if let Some(addr) = payload.get("username").and_then(|v| v.as_str()) {
        format!("{}.json", addr.replace('@', "_at_"))
    } else {
        "account.json".to_string()
    };

    let json_content = serde_json::to_string(&payload).unwrap_or_default();

    // 创建 multipart form
    // 关键点：CLIProxyAPI 要求字段名为 "file"，且内容为 JSON 文件
    let part = reqwest::multipart::Part::text(json_content)
        .file_name(filename)
        .mime_str("application/json")
        .map_err(|e| format!("构建 Part 失败: {}", e))?;

    let form = reqwest::multipart::Form::new().part("file", part);

    let res = client
        .post(cpa_url)
        .header("Authorization", format!("Bearer {}", cpa_key))
        .header("x-management-key", cpa_key) // 兼容 CLIProxyAPI 的 Header
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Multipart 分发失败: {}", e))?;

    if res.status().is_success() {
        Ok(())
    } else {
        let status = res.status();
        let err_body = res.text().await.unwrap_or_default();
        Err(format!("CPA 平台拒绝 ({}): {}", status, err_body))
    }
}
