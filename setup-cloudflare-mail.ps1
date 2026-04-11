param(
    [ValidateSet("auto", "local_trycloudflare", "public_ip", "public_domain")]
    [string]$Mode = "auto",
    [string]$PublicUrl,
    [string]$HubSecret = "local_dev_secret",
    [string]$RouteLocalPart = "inbox",
    [string]$ZoneDomain = $env:CLOUDFLARE_ZONE_DOMAIN,
    [string]$CloudflareApiToken = $env:CLOUDFLARE_API_TOKEN,
    [string]$CloudflareZoneId = $env:CLOUDFLARE_ZONE_ID,
    [string]$CloudflareAccountId = $env:CLOUDFLARE_ACCOUNT_ID,
    [switch]$SkipWorkerDeploy,
    [switch]$SkipRoutingRule,
    [switch]$SkipPublicIngestTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectRoot = $PSScriptRoot
$networkDir = Join-Path $projectRoot "network"
$stateDir = Join-Path $projectRoot ".automation"
$wranglerTomlPath = Join-Path $networkDir "wrangler.toml"
$configPath = Join-Path $stateDir "cloudflare-config.json"
$workerName = "phantom-drop-edge"

New-Item -ItemType Directory -Force -Path $stateDir | Out-Null

function Import-AutomationConfig {
    if (-not (Test-Path $configPath)) {
        return @{}
    }

    $raw = Get-Content $configPath -Raw
    if ([string]::IsNullOrWhiteSpace($raw)) {
        return @{}
    }

    $parsed = $raw | ConvertFrom-Json
    $config = @{}
    foreach ($property in $parsed.PSObject.Properties) {
        $config[$property.Name] = $property.Value
    }
    return $config
}

function Save-AutomationConfig([hashtable]$Config) {
    ($Config | ConvertTo-Json -Depth 10) | Set-Content -LiteralPath $configPath -Encoding UTF8
}

function Select-Value([object[]]$Candidates) {
    foreach ($candidate in $Candidates) {
        if ($null -eq $candidate) {
            continue
        }
        if ($candidate -is [string]) {
            if (-not [string]::IsNullOrWhiteSpace($candidate)) {
                return $candidate
            }
            continue
        }
        return $candidate
    }
    return $null
}

function Write-Step([string]$Message) {
    Write-Host "[STEP] $Message" -ForegroundColor Cyan
}

function Write-Info([string]$Message) {
    Write-Host "[INFO] $Message" -ForegroundColor DarkCyan
}

function Write-Ok([string]$Message) {
    Write-Host "[ OK ] $Message" -ForegroundColor Green
}

function Write-Warn([string]$Message) {
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Test-LocalBackend {
    try {
        $null = Invoke-RestMethod -Uri "http://127.0.0.1:9010/health" -TimeoutSec 5
        return $true
    } catch {
        return $false
    }
}

function Get-BackendAutomationConfig {
    if (-not (Test-LocalBackend)) {
        return @{}
    }

    try {
        $settings = Invoke-RestMethod -Uri "http://127.0.0.1:9010/api/settings" -TimeoutSec 10
        $config = @{}
        foreach ($property in $settings.PSObject.Properties) {
            $config[$property.Name] = $property.Value
        }
        return $config
    } catch {
        return @{}
    }
}

function Resolve-ModeValue([string]$RequestedMode, [string]$CandidateUrl) {
    if ($RequestedMode -ne "auto") {
        return $RequestedMode
    }

    if ([string]::IsNullOrWhiteSpace($CandidateUrl)) {
        return "local_trycloudflare"
    }

    $normalized = $CandidateUrl.Trim()
    if ($normalized -notmatch "^https?://") {
        if ($normalized -match "^\d{1,3}(\.\d{1,3}){3}(:\d+)?$") {
            return "public_ip"
        }
        return "public_domain"
    }

    $uri = [Uri]$normalized
    if ($uri.Host -like "*.trycloudflare.com") {
        return "local_trycloudflare"
    }
    if ($uri.Host -match "^\d{1,3}(\.\d{1,3}){3}$") {
        return "public_ip"
    }
    return "public_domain"
}

function Format-PublicUrl([string]$ModeValue, [string]$CandidateUrl) {
    if ([string]::IsNullOrWhiteSpace($CandidateUrl)) {
        return $null
    }

    $trimmed = $CandidateUrl.Trim().TrimEnd("/")
    if ($trimmed -notmatch "^https?://") {
        if ($ModeValue -eq "public_ip") {
            $trimmed = "http://$trimmed"
        } else {
            $trimmed = "https://$trimmed"
        }
    }

    $uri = [Uri]$trimmed
    return $uri.GetLeftPart([System.UriPartial]::Authority)
}

function Start-QuickTunnel {
    if (-not (Get-Command cloudflared -ErrorAction SilentlyContinue)) {
        throw "cloudflared is not installed or not available in PATH."
    }
    if (-not (Test-LocalBackend)) {
        throw "Local backend on http://127.0.0.1:9010 is not reachable."
    }

    $stdoutPath = Join-Path $stateDir "cloudflared.stdout.log"
    $stderrPath = Join-Path $stateDir "cloudflared.stderr.log"
    Remove-Item -LiteralPath $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue

    $process = Start-Process -FilePath "cloudflared" `
        -ArgumentList @("tunnel", "--url", "http://127.0.0.1:9010", "--protocol", "http2", "--edge-ip-version", "4") `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath `
        -PassThru `
        -WindowStyle Hidden

    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep -Seconds 2

        if ($process.HasExited) {
            $stderr = if (Test-Path $stderrPath) { Get-Content $stderrPath -Raw } else { "" }
            throw "cloudflared exited early. $stderr"
        }

        $stdout = if (Test-Path $stdoutPath) { Get-Content $stdoutPath -Raw } else { "" }
        $stderr = if (Test-Path $stderrPath) { Get-Content $stderrPath -Raw } else { "" }
        $combined = "$stdout`n$stderr"
        $match = [regex]::Match($combined, "https://[a-z0-9-]+\.trycloudflare\.com")
        if ($match.Success) {
            return [PSCustomObject]@{
                Url = $match.Value
                ProcessId = $process.Id
                StdoutLog = $stdoutPath
                StderrLog = $stderrPath
            }
        }
    }

    throw "Timed out waiting for cloudflared to publish a trycloudflare.com URL."
}

function Test-HealthEndpoint([string]$BaseUrl, [string]$Label) {
    $target = "$BaseUrl/health"
    Write-Step "Checking $Label health at $target"
    $response = Invoke-RestMethod -Uri $target -TimeoutSec 20
    Write-Ok "$Label health check succeeded."
    return $response
}

function Save-BackendRegistration([string]$BaseUrl) {
    if (-not (Test-LocalBackend)) {
        Write-Warn "Local backend is not running. Skipping local console registration."
        return
    }

    Write-Step "Saving public URL into local PhantomDrop settings"
    $settingsBody = @{
        public_hub_url = $BaseUrl
    } | ConvertTo-Json -Depth 5

    $null = Invoke-RestMethod `
        -Uri "http://127.0.0.1:9010/api/settings/save" `
        -Method Post `
        -ContentType "application/json; charset=utf-8" `
        -Body $settingsBody

    $tunnelBody = @{
        port = 9010
        public_url = $BaseUrl
    } | ConvertTo-Json -Depth 5

    $null = Invoke-RestMethod `
        -Uri "http://127.0.0.1:9010/api/tunnel/start" `
        -Method Post `
        -ContentType "application/json; charset=utf-8" `
        -Body $tunnelBody

    Write-Ok "Local settings and tunnel registration updated."
}

function Update-WranglerToml([string]$BaseUrl, [string]$Secret) {
    Write-Step "Updating network/wrangler.toml"
    $content = Get-Content $wranglerTomlPath -Raw
    $content = [regex]::Replace($content, '(?m)^PHANTOM_HUB_URL\s*=\s*".*"$', "PHANTOM_HUB_URL = `"$BaseUrl`"")
    $content = [regex]::Replace($content, '(?m)^HUB_SECRET\s*=\s*".*"$', "HUB_SECRET = `"$Secret`"")
    if ($content -notmatch '(?m)^workers_dev\s*=\s*true$') {
        $content = $content -replace '(?m)^compatibility_date\s*=\s*".*"$', "$0`r`nworkers_dev = true"
    }
    $content | Set-Content -LiteralPath $wranglerTomlPath -Encoding UTF8
    Write-Ok "wrangler.toml updated."
}

function Initialize-NetworkDependencies {
    if (Test-Path (Join-Path $networkDir "node_modules")) {
        Write-Info "network/node_modules already exists. Skipping npm install."
        return
    }

    Write-Step "Installing network dependencies"
    Push-Location $networkDir
    try {
        & npm install
        if ($LASTEXITCODE -ne 0) {
            throw "npm install failed."
        }
    } finally {
        Pop-Location
    }
    Write-Ok "network dependencies are ready."
}

function Invoke-WranglerDeploy([string]$ApiToken) {
    Write-Step "Deploying Cloudflare worker"
    
    # 注入环境变量以支持非交互式部署
    if (-not [string]::IsNullOrWhiteSpace($ApiToken)) {
        $env:CLOUDFLARE_API_TOKEN = $ApiToken
    }

    Push-Location $networkDir
    try {
        $output = (& npx wrangler deploy 2>&1) | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw $output
        }
    } finally {
        Pop-Location
    }

    $workerUrl = $null
    $match = [regex]::Match($output, 'https://[a-z0-9.-]+\.workers\.dev')
    if ($match.Success) {
        $workerUrl = $match.Value
    }

    Write-Ok "Worker deployment finished."
    return [PSCustomObject]@{
        Output = $output
        WorkerUrl = $workerUrl
    }
}

function Resolve-ZoneDomain([string]$ExplicitDomain, [string]$BaseUrl) {
    if (-not [string]::IsNullOrWhiteSpace($ExplicitDomain)) {
        return $ExplicitDomain.Trim().ToLowerInvariant()
    }

    $urlHost = ([Uri]$BaseUrl).Host
    $parts = $urlHost.Split('.')
    if ($parts.Length -ge 2) {
        return "$($parts[$parts.Length - 2]).$($parts[$parts.Length - 1])"
    }
    return $urlHost
}

function Get-WranglerAccountId([string]$CurrentAccountId) {
    if (-not [string]::IsNullOrWhiteSpace($CurrentAccountId)) {
        return $CurrentAccountId
    }

    Push-Location $networkDir
    try {
        $whoami = & npx wrangler whoami --json 2>$null
        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($whoami)) {
            return $null
        }
        $parsed = $whoami | ConvertFrom-Json
        if ($parsed.accounts.Count -gt 0) {
            return $parsed.accounts[0].id
        }
        return $null
    } finally {
        Pop-Location
    }
}

function Invoke-CloudflareApi([string]$Method, [string]$Path, [object]$Body) {
    if ([string]::IsNullOrWhiteSpace($CloudflareApiToken)) {
        throw "CLOUDFLARE_API_TOKEN is required for routing rule automation."
    }

    $headers = @{
        Authorization = "Bearer $CloudflareApiToken"
    }

    $params = @{
        Uri = "https://api.cloudflare.com/client/v4$Path"
        Method = $Method
        Headers = $headers
    }

    if ($null -ne $Body) {
        $params.ContentType = "application/json; charset=utf-8"
        $params.Body = $Body | ConvertTo-Json -Depth 10
    }

    return Invoke-RestMethod @params
}

function Set-EmailRoutingRule([string]$EmailAddress) {
    if ([string]::IsNullOrWhiteSpace($CloudflareZoneId)) {
        Write-Warn "CLOUDFLARE_ZONE_ID is missing. Skipping routing rule automation."
        return $null
    }

    $resolvedAccountId = Get-WranglerAccountId -CurrentAccountId $CloudflareAccountId
    if ([string]::IsNullOrWhiteSpace($resolvedAccountId)) {
        Write-Warn "Cloudflare account id could not be resolved. Skipping routing rule automation."
        return $null
    }

    Write-Step "Ensuring Email Routing rule for $EmailAddress"
    $rulesResponse = Invoke-CloudflareApi -Method "GET" -Path "/zones/$CloudflareZoneId/email/routing/rules" -Body $null
    $existingRule = $rulesResponse.result | Where-Object {
        $_.matchers | Where-Object { $_.field -eq "to" -and $_.value -eq $EmailAddress }
    } | Select-Object -First 1

    $payload = @{
        name = "PhantomDrop Email Worker"
        enabled = $true
        priority = 0
        matchers = @(
            @{
                type = "literal"
                field = "to"
                value = $EmailAddress
            }
        )
        actions = @(
            @{
                type = "worker"
                value = @($workerName)
            }
        )
    }

    if ($null -ne $existingRule) {
        $null = Invoke-CloudflareApi -Method "PUT" -Path "/zones/$CloudflareZoneId/email/routing/rules/$($existingRule.id)" -Body $payload
        Write-Ok "Updated existing Email Routing rule."
        return $existingRule.id
    }

    $createdRule = Invoke-CloudflareApi -Method "POST" -Path "/zones/$CloudflareZoneId/email/routing/rules" -Body $payload
    Write-Ok "Created Email Routing rule."
    return $createdRule.result.id
}

function Invoke-PublicIngestSmokeTest([string]$BaseUrl, [string]$Secret) {
    Write-Step "Running public ingest smoke test through $BaseUrl/ingest"
    $subject = "PhantomDrop Auto Ingest Probe $(Get-Date -Format 'yyyyMMdd-HHmmss')"
    $payload = @{
        meta = @{
            from = "automation-probe@phantomdrop.local"
            to = "probe@phantomdrop.local"
            subject = $subject
            date = (Get-Date).ToUniversalTime().ToString("o")
        }
        content = @{
            text = "Automation probe code 135790."
            html = "<html><body><p>Automation probe code <strong>135790</strong>.</p></body></html>"
        }
    } | ConvertTo-Json -Depth 10

    $response = Invoke-WebRequest `
        -Uri "$BaseUrl/ingest" `
        -Method Post `
        -Headers @{ "X-Hub-Secret" = $Secret } `
        -ContentType "application/json; charset=utf-8" `
        -Body $payload

    if ($response.StatusCode -ne 200) {
        throw "Public ingest smoke test failed with status $($response.StatusCode)."
    }

    Write-Ok "Public ingest smoke test succeeded."
    return $subject
}

function Invoke-WorkerSmokeTest([string]$WorkerUrl) {
    if ([string]::IsNullOrWhiteSpace($WorkerUrl)) {
        Write-Warn "未从部署输出中检测到 Worker URL，跳过冒烟测试。"
        return
    }

    # 健康检查
    Write-Step "Checking deployed worker health at $WorkerUrl/health"
    try {
        $null = Invoke-RestMethod -Uri "$WorkerUrl/health" -TimeoutSec 20
        Write-Ok "Worker 健康检查通过。"
    } catch {
        Write-Warn "Worker 健康检查失败（可能是冷启动延迟）：$($_.Exception.Message)"
    }

    # 中继测试（非关键，失败不阻断流程）
    Write-Step "Running worker relay smoke test at $WorkerUrl/relay-test"
    try {
        $null = Invoke-RestMethod `
            -Uri "$WorkerUrl/relay-test" `
            -Method Post `
            -ContentType "application/json; charset=utf-8" `
            -Body "{}" `
            -TimeoutSec 30
        Write-Ok "Worker 中继测试通过。"
    } catch {
        Write-Warn "Worker 中继测试未通过（错误：$($_.Exception.Message)）。这不影响 Worker 部署，后续真实邮件到达时会自动中继。"
    }
}

Write-Host "========================================================"
Write-Host "PhantomDrop Cloudflare Mail Automation"
Write-Host "Project   : $projectRoot"
Write-Host "Mode      : $Mode"
Write-Host "========================================================"

$automationConfig = Import-AutomationConfig
$backendAutomationConfig = Get-BackendAutomationConfig

$effectiveHubSecret = if ($PSBoundParameters.ContainsKey("HubSecret")) {
    $HubSecret
} else {
    Select-Value @($backendAutomationConfig['auth_secret'], $automationConfig['hub_secret'], $HubSecret)
}

$effectiveRouteLocalPart = if ($PSBoundParameters.ContainsKey("RouteLocalPart")) {
    $RouteLocalPart
} else {
    Select-Value @($backendAutomationConfig['cloudflare_route_local_part'], $automationConfig['route_local_part'], $RouteLocalPart, "inbox")
}
$effectiveZoneDomain = Select-Value @($ZoneDomain, $backendAutomationConfig['cloudflare_zone_domain'], $automationConfig['zone_domain'])
$effectiveCloudflareApiToken = Select-Value @($CloudflareApiToken, $backendAutomationConfig['cloudflare_api_token'], $automationConfig['cloudflare_api_token'])
$effectiveCloudflareZoneId = Select-Value @($CloudflareZoneId, $backendAutomationConfig['cloudflare_zone_id'], $automationConfig['cloudflare_zone_id'])
$effectiveCloudflareAccountId = Select-Value @($CloudflareAccountId, $backendAutomationConfig['cloudflare_account_id'], $automationConfig['cloudflare_account_id'])
$configuredDefaultMode = Select-Value @($backendAutomationConfig['cloudflare_default_mode'], $automationConfig['default_mode'])
$configuredDefaultPublicUrl = Select-Value @($backendAutomationConfig['cloudflare_public_url'], $automationConfig['default_public_url'])

if ($Mode -eq "auto" -and -not [string]::IsNullOrWhiteSpace($configuredDefaultMode)) {
    $Mode = [string]$configuredDefaultMode
}

if ([string]::IsNullOrWhiteSpace($PublicUrl) -and -not [string]::IsNullOrWhiteSpace($configuredDefaultPublicUrl)) {
    $PublicUrl = [string]$configuredDefaultPublicUrl
}

$resolvedMode = Resolve-ModeValue -RequestedMode $Mode -CandidateUrl $PublicUrl
$quickTunnel = $null

if ($resolvedMode -eq "local_trycloudflare" -and [string]::IsNullOrWhiteSpace($PublicUrl)) {
    Write-Step "Creating a quick trycloudflare tunnel"
    $quickTunnel = Start-QuickTunnel
    $PublicUrl = $quickTunnel.Url
    Write-Ok "Quick tunnel is ready at $PublicUrl"
}

$normalizedPublicUrl = Format-PublicUrl -ModeValue $resolvedMode -CandidateUrl $PublicUrl
if ([string]::IsNullOrWhiteSpace($normalizedPublicUrl)) {
    throw "A public URL is required for this mode."
}

$null = Test-HealthEndpoint -BaseUrl $normalizedPublicUrl -Label "Public hub"
Save-BackendRegistration -BaseUrl $normalizedPublicUrl
Update-WranglerToml -BaseUrl $normalizedPublicUrl -Secret $effectiveHubSecret

$deployResult = $null
if (-not $SkipWorkerDeploy) {
    Initialize-NetworkDependencies
    $deployResult = Invoke-WranglerDeploy -ApiToken $effectiveCloudflareApiToken
    Invoke-WorkerSmokeTest -WorkerUrl $deployResult.WorkerUrl
}

$routingRuleId = $null
$resolvedZoneDomain = $null
$emailAddress = $null
if (-not $SkipRoutingRule) {
    $CloudflareApiToken = $effectiveCloudflareApiToken
    $CloudflareZoneId = $effectiveCloudflareZoneId
    $CloudflareAccountId = $effectiveCloudflareAccountId
    $resolvedZoneDomain = Resolve-ZoneDomain -ExplicitDomain $effectiveZoneDomain -BaseUrl $normalizedPublicUrl
    $emailAddress = "$effectiveRouteLocalPart@$resolvedZoneDomain"
    try {
        $routingRuleId = Set-EmailRoutingRule -EmailAddress $emailAddress
    } catch {
        Write-Warn "Routing rule automation failed: $($_.Exception.Message)"
    }
} elseif (-not [string]::IsNullOrWhiteSpace($effectiveZoneDomain)) {
    $resolvedZoneDomain = Resolve-ZoneDomain -ExplicitDomain $effectiveZoneDomain -BaseUrl $normalizedPublicUrl
    $emailAddress = "$effectiveRouteLocalPart@$resolvedZoneDomain"
}

$probeSubject = $null
if (-not $SkipPublicIngestTest) {
    $probeSubject = Invoke-PublicIngestSmokeTest -BaseUrl $normalizedPublicUrl -Secret $effectiveHubSecret
}

$automationConfig.default_mode = $resolvedMode
if ($resolvedMode -ne "local_trycloudflare") {
    $automationConfig.default_public_url = $normalizedPublicUrl
}
if (-not [string]::IsNullOrWhiteSpace($effectiveHubSecret)) {
    $automationConfig.hub_secret = $effectiveHubSecret
}
if (-not [string]::IsNullOrWhiteSpace($effectiveRouteLocalPart)) {
    $automationConfig.route_local_part = $effectiveRouteLocalPart
}
if (-not [string]::IsNullOrWhiteSpace($effectiveZoneDomain)) {
    $automationConfig.zone_domain = $effectiveZoneDomain
}
if (-not [string]::IsNullOrWhiteSpace($effectiveCloudflareApiToken)) {
    $automationConfig.cloudflare_api_token = $effectiveCloudflareApiToken
}
if (-not [string]::IsNullOrWhiteSpace($effectiveCloudflareZoneId)) {
    $automationConfig.cloudflare_zone_id = $effectiveCloudflareZoneId
}
if (-not [string]::IsNullOrWhiteSpace($effectiveCloudflareAccountId)) {
    $automationConfig.cloudflare_account_id = $effectiveCloudflareAccountId
}
Save-AutomationConfig -Config $automationConfig

$summary = [PSCustomObject]@{
    mode = $resolvedMode
    public_url = $normalizedPublicUrl
    quick_tunnel_pid = if ($quickTunnel) { $quickTunnel.ProcessId } else { $null }
    worker_url = if ($deployResult) { $deployResult.WorkerUrl } else { $null }
    routing_rule_id = $routingRuleId
    zone_domain = $resolvedZoneDomain
    route_local_part = $effectiveRouteLocalPart
    email_address = $emailAddress
    probe_subject = $probeSubject
}

$summaryPath = Join-Path $stateDir "cloudflare-mail-last-run.json"
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $summaryPath -Encoding UTF8

Write-Host ""
Write-Host "Automation completed." -ForegroundColor Green
$summary | Format-List
Write-Host ""
Write-Host "Summary saved to $summaryPath" -ForegroundColor DarkCyan
