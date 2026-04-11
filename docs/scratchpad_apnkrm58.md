# Repository Exploration: codex-console

## Goal
Understand the structure and implementation of `https://github.com/dou-jiang/codex-console`.

## Summary
- **Purpose**: A comprehensive console for OpenAI account registration, management, batch processing, and token handling. It is a fork of `cnlimiter/codex-manager`, specialized in fixing registration issues.
- **Tech Stack**:
  - Backend: Python 3.10+, FastAPI, Uvicorn.
  - Database: SQLAlchemy, Alembic (defaulting to SQLite).
  - HTTP: `curl_cffi` (to bypass TLS fingerprinting).
  - Browser: Playwright (used in `src/core/openai/browser_bind.py`).
  - Frontend: HTML/JS templates (FastAPI Jinja2).
- **Core Architecture**:
  - `webui.py`: Entry point for the web application.
  - `src/web`: FastAPI application, routes, and task management.
  - `src/core`: Core logic.
    - `openai/`: OpenAI-specific flows including PoW (`sentinel.py`), OAuth (`oauth.py`), and payment.
    - `anyauto/`: Registration flow inspired by or using `any-auto-register`.
    - `register.py`: Main engine orchestrating registration tasks.
- **Key Files & Roles**:
  - `src/core/openai/sentinel.py`: Implements PoW (Proof of Work) solving for OpenAI using SHA3-512.
  - `src/core/openai/oauth.py`: Handles OAuth PKCE flow, state management, and token exchange.
  - `src/core/anyauto/register_flow.py`: Impements a state-machine based registration flow.
  - `src/web/routes/registration.py`: API endpoints for starting and monitoring registration tasks.
  - `.env.example`: Configures database, port, and third-party API integration (e.g., card binding).
- **Patterns**:
  - Uses a task management system (`src/web/task_manager.py`) for background registration tasks.
  - Extensive use of `curl_cffi` for all OpenAI requests to appear as a real browser.
- **Registration Logic**:
  - Supports multiple registration flows (default and anyauto).
  - Handles email verification via various email service adapters.
  - Solves OpenAI's "Sentinel" (PoW) challenges.

## Links Investigated
- [x] README.md
- [x] src/core/register.py
- [x] src/core/openai/sentinel.py
- [x] src/core/openai/oauth.py
- [x] src/core/anyauto/register_flow.py
- [x] src/web/app.py
- [x] src/web/routes/registration.py
- [x] .env.example
- [x] Dockerfile
