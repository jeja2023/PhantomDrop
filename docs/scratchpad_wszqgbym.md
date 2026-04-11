# 任务：向 Cloudflare Worker 发送测试请求

## 计划
1. [x] 在浏览器中执行 JavaScript 发送 POST 请求到 `https://phantom-drop-edge.yunjiankai.workers.dev/relay-test`。
   - Body: `{}`
   - Headers: `Content-Type: application/json`
2. [x] 记录并报告状态码和响应内容。

## 进展
- 请求已完成。
- 状态码：200
- 响应内容：
  ```json
  {
    "status": "success",
    "hub_status": 200,
    "hub_response": "邮件已注入",
    "forwarded_subject": "PhantomDrop Worker Probe 2026-04-11T09-04-20-399Z"
  }
  ```
