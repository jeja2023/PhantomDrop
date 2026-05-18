use crate::db::DataLake;
use std::sync::Arc;

/// 启动代理质量心跳检测后台循环
pub fn start_proxy_heartbeat_loop(data_lake: Arc<DataLake>) {
    tokio::spawn(async move {
        println!("🚀 [代理心跳] 代理质量与 RTT 心跳检测器已启动，每 5 分钟轮询一次...");
        loop {
            // 每 5 分钟执行一次心跳检测
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;

            // 1. 读取数据库中所有绑定了代理的账号记录
            let accounts = match data_lake.list_all_accounts_with_proxies().await {
                Ok(accs) => accs,
                Err(e) => {
                    eprintln!("🔴 [代理心跳] 无法读取代理账号列表: {:?}", e);
                    continue;
                }
            };

            for account in accounts {
                let proxy_url = match &account.proxy_url {
                    Some(p) if !p.trim().is_empty() => p.clone(),
                    _ => continue,
                };

                let data_lake_clone = data_lake.clone();
                let account_id = account.id.clone();

                // 异步多线程并行检测，不阻塞主轮询循环
                tokio::spawn(async move {
                    let start = std::time::Instant::now();
                    let client = crate::openai::impersonator::ImpersonateProvider::create_chrome_client(Some(&proxy_url));

                    let check_fut = crate::openai::sentinel::check_ip_quality(&client);
                    match tokio::time::timeout(tokio::time::Duration::from_secs(10), check_fut).await {
                        Ok(Ok(info)) => {
                            let rtt = start.elapsed().as_millis() as i64;
                            let ip_type = if info.is_datacenter { "datacenter" } else { "residential" };
                            let _ = data_lake_clone.update_proxy_quality(&account_id, rtt, ip_type, "active").await;
                            println!(
                                "✅ [代理心跳] 账号 {} 检测成功 | IP: {} | 延迟: {}ms | 类型: {}",
                                account_id, info.ip, rtt, ip_type
                            );
                        }
                        Ok(Err(e)) => {
                            // 检测失败，标记为离线
                            let _ = data_lake_clone.update_proxy_quality(&account_id, 9999, "unknown", "offline").await;
                            eprintln!("⚠️ [代理心跳] 账号 {} 检测失败 (已标为离线) | 错误: {}", account_id, e);
                        }
                        Err(_) => {
                            // 超时，标记为离线
                            let _ = data_lake_clone.update_proxy_quality(&account_id, 9999, "unknown", "offline").await;
                            eprintln!("⚠️ [代理心跳] 账号 {} 检测超时 (10秒超时，已标为离线)", account_id);
                        }
                    }
                });

                // 每个检测任务间加入小幅休眠，避免突发高频打满代理提供商或 CPU 资源
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            }
        }
    });
}
