# CPACM Registration Mechanism Analysis

## Findings
- **Project**: CPACM (CPA-Codex-Manager)
- **Core Functionality**: Automated OpenAI account registration and management.
- **Key Technology Components**:
    - **`curl_cffi`**: Used for TLS/browser fingerprint impersonation (JA3/unja3) to bypass Cloudflare and other bot detection systems.
    - **`RegistrationEngine` (`src/core/register.py`)**: Orchestrates the entire registration process.
    - **`OAuthManager` (`src/core/openai/oauth.py`)**: Handles the PKCE (Proof Key for Code Exchange) flow, including state generation, PKCE challenge/verifier, and token exchange.
    - **`Sentinel` (`src/core/openai/sentinel.py`)**: Crucial for bypassing OpenAI's latest bot protection. It:
        - Emulates browser fingerprinting (screen, navigator, window properties).
        - Solves SHA3-512 Proof-of-Work (PoW) challenges required by OpenAI's "sentinel" system.
- **Registration Flow**:
    1. **Initialization**: Start OAuth flow, generate PKCE parameters and session state.
    2. **Email Submission**: Submit the target email for signup via OpenAI's auth endpoints.
    3. **Email Verification**: User/system verifies the email (likely through a catch-all service or specific email API).
    4. **Password Submission**: Submit the desired password for the verified email.
    5. **Bot Detection Bypass**:
        - Fetch a "sentinel" challenge from OpenAI.
        - Solve the PoW challenge using local CPU (implemented in `sentinel.py`).
        - Submit the solved PoW and browser fingerprint data.
    6. **Token Acquisition**: Exchange the resulting authorization code for `access_token` and `refresh_token`.

## Conclusion
The registration mechanism is a highly sophisticated, purely HTTP-based (non-Headless browser) approach that carefully emulates browser behavior and solves complex security challenges (PoW, TLS fingerprinting) to achieve high success rates and throughput.
