# Any Auto Register Findings

## Key Features
- **Registration Logic**: Supports multiple platforms including ChatGPT. Uses API protocols (no browser) as the primary mode.
- **Captcha Solving**: Integrates YesCaptcha, 2Captcha, and a local solver (Camoufox).
- **Proxy Management**: Features auto-rotation, success rate tracking, and automatic disabling of failed proxies.
- **Email Verification**: Supports MoeMail (self-hosted), Laoudo, DuckMail, and Cloudflare Worker based self-hosted emails.
- **Architecture**: Domain-Driven Design (DDD) with FastAPI and SQLite.

## Project Structure (Observed)
- `platforms/`: Individual platform registration logic.
- `services/`: Core services like captcha, email, etc.
- `infrastructure/`: Database, proxy, and other external integrations.

## any-auto-register Detail Findings
- **Registration Logic**:
    - Uses `curl_cffi` for HTTP/Protocol-based registration (OpenAI).
    - Implements Sentinel check and OAuth flow (`platforms/chatgpt/register.py`).
    - Provides both protocol and browser-based registration options.
- **Captcha Solving**:
    - Integrates with YesCaptcha and 2Captcha.
    - Features a local solver using `Camoufox` (anti-detect browser) and `Patchright` (anti-detect Playwright) for Turnstile.
    - Captcha solving is managed as a separate service/process (`services/turnstile_solver`).
- **Proxy Management**:
    - Domain-level logic for proxy health and rotation.
    - Supports auto-rotation and failure-based disabling.
- **Email Verification**:
    - Supports multiple providers: MoeMail, Laoudo, DuckMail, and self-hosted Cloudflare Workers.
- **Account Upload**:
    - Explicit logic for uploading to CPA (`platforms/chatgpt/cpa_upload.py`) using `multipart/form-data`.
- **Unique Features**:
    - DDD architecture.
    - Local anti-detect browser integration (`Camoufox`).
    - Comprehensive task logging and success rate tracking.
