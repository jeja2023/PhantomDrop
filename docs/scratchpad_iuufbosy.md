# Any Auto Register Exploration

## Project Purpose
Automatic registration tool for multiple platforms (ChatGPT, Cursor, Trae, etc.).

## Technology Stack
- **Backend:** Python 3.11+, FastAPI, SQLModel (SQLite).
- **Frontend:** Node.js 18+, React/Vite (implied).
- **Other:** Electron (likely for desktop app), Proxy support, Captcha solvers (Turnstile, etc.).

## Directory Structure (Complete)
- `api/`: FastAPI endpoints (e.g., account management, task control).
- `application/`: Application services.
    - `accounts.py`: Account CRUD and management.
    - `actions.py`: Orchestrates execution of platform actions.
    - `platforms.py`: Manages platform metadata and capabilities.
    - `provider_settings.py`: Configuration for email, captcha, and proxy providers.
- `core/`: Core internal utilities.
- `domain/`: Data models (SQLModel/SQLite).
- `infrastructure/`: Data persistence (repositories) and external integrations.
- `platforms/`: Platform-specific registration logic.
    - `chatgpt/`: ChatGPT registration (protocol and browser modes).
        - `register.py`: Main protocol-based registration engine.
        - `oauth.py`: OAuth flow handling.
        - `protocol_mailbox.py`: Handles mail OTP extraction.
    - `cursor/`: Cursor registration logic.
    - `trae/`: Trae registration logic.
- `services/`: Specialized components.
    - `turnstile_solver/`: Powerful Turnstile solver using `camoufox` and `patchright`.
    - `task_runtime/`: Background task management.
- `frontend/`: React/Vite development source.
- `electron/`: Desktop application source.

## Key Implementation Details
- **OpenAI/ChatGPT Registration:**
    - **Protocol Mode:** Uses `curl_cffi` for TLS fingerprint mimicking.
    - **Flow:** `_init_session` -> `_start_oauth` -> `_check_sentinel` (checks for PoW challenge) -> `_submit_signup_form` -> `_register_password` -> `_wait_for_email_verification` -> `_validate_verification_code` -> `_create_user_account`.
- **Captcha Handling (Turnstile):**
    - Implemented as a microservice in `services/turnstile_solver/api_solver.py`.
    - Uses `camoufox` for anti-bot browser profiles and `patchright` (patched Playwright).
    - Features shadow DOM injection to capture and interact with Turnstile tokens.
- **Account Management:**
    - Centralized in `application/accounts.py` using repository pattern for SQLite persistence.
- **Configuration:**
    - Managed via `infrastructure/config_repository.py` and `provider_settings.py`.
    - Supports multiple captcha providers (YesCaptcha, 2Captcha) and email services.

## Important Patterns
- **Repository Pattern:** Decouples domain models from database logic.
- **Service Layer:** `application/` services coordinate the flow between domain and infrastructure.
- **Pluggable Platforms:** Each platform in `platforms/` is a modular unit with its own registration logic.
