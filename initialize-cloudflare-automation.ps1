param(
    [ValidateSet("local_trycloudflare", "public_ip", "public_domain")]
    [string]$DefaultMode = "public_domain",
    [string]$DefaultPublicUrl,
    [string]$HubSecret = "local_dev_secret",
    [string]$RouteLocalPart = "inbox",
    [string]$ZoneDomain,
    [string]$CloudflareApiToken,
    [string]$CloudflareZoneId,
    [string]$CloudflareAccountId,
    [switch]$RunWranglerLogin,
    [switch]$NoDialog
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectRoot = $PSScriptRoot
$networkDir = Join-Path $projectRoot "network"
$stateDir = Join-Path $projectRoot ".automation"
$configPath = Join-Path $stateDir "cloudflare-config.json"

New-Item -ItemType Directory -Force -Path $stateDir | Out-Null

function Load-Config {
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

function Save-Config([hashtable]$Config) {
    ($Config | ConvertTo-Json -Depth 10) | Set-Content -LiteralPath $configPath -Encoding UTF8
}

function Select-Value([object[]]$Candidates) {
    foreach ($candidate in $Candidates) {
        if ($null -ne $candidate) {
            if ($candidate -is [string]) {
                if (-not [string]::IsNullOrWhiteSpace($candidate)) {
                    return $candidate
                }
                continue
            }
            return $candidate
        }
    }
    return $null
}

function Show-InitializationDialog([hashtable]$Defaults) {
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing

    $form = New-Object System.Windows.Forms.Form
    $form.Text = "PhantomDrop Cloudflare 首次授权"
    $form.StartPosition = "CenterScreen"
    $form.Size = New-Object System.Drawing.Size(720, 560)
    $form.FormBorderStyle = "FixedDialog"
    $form.MaximizeBox = $false
    $form.MinimizeBox = $false

    $font = New-Object System.Drawing.Font("Microsoft YaHei UI", 9)
    $title = New-Object System.Windows.Forms.Label
    $title.Text = "首次授权后，后续可直接运行 .\setup-cloudflare-mail.ps1"
    $title.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 10, [System.Drawing.FontStyle]::Bold)
    $title.Location = New-Object System.Drawing.Point(20, 16)
    $title.Size = New-Object System.Drawing.Size(650, 24)
    $form.Controls.Add($title)

    $fields = @(
        @{ Label = "默认模式"; Key = "default_mode"; Type = "combo"; Values = @("public_domain", "public_ip", "local_trycloudflare") },
        @{ Label = "默认公网地址"; Key = "default_public_url"; Type = "text" },
        @{ Label = "收件地址前缀"; Key = "route_local_part"; Type = "text" },
        @{ Label = "主域名"; Key = "zone_domain"; Type = "text" },
        @{ Label = "Hub Secret"; Key = "hub_secret"; Type = "text" },
        @{ Label = "Cloudflare API Token"; Key = "cloudflare_api_token"; Type = "text" },
        @{ Label = "Cloudflare Zone ID"; Key = "cloudflare_zone_id"; Type = "text" },
        @{ Label = "Cloudflare Account ID"; Key = "cloudflare_account_id"; Type = "text" }
    )

    $controls = @{}
    $top = 56
    foreach ($field in $fields) {
        $label = New-Object System.Windows.Forms.Label
        $label.Text = $field.Label
        $label.Font = $font
        $label.Location = New-Object System.Drawing.Point(20, $top)
        $label.Size = New-Object System.Drawing.Size(180, 22)
        $form.Controls.Add($label)

        if ($field.Type -eq "combo") {
            $control = New-Object System.Windows.Forms.ComboBox
            $control.DropDownStyle = "DropDownList"
            [void]$control.Items.AddRange($field.Values)
            $selected = [string]$Defaults[$field.Key]
            if ([string]::IsNullOrWhiteSpace($selected)) {
                $selected = ""
            }
            if ([string]::IsNullOrWhiteSpace($selected)) {
                $selected = $field.Values[0]
            }
            $control.SelectedItem = $selected
        } else {
            $control = New-Object System.Windows.Forms.TextBox
            $val = [string]$Defaults[$field.Key]
            if ([string]::IsNullOrWhiteSpace($val)) {
                $val = ""
            }
            $control.Text = $val
        }

        $control.Font = $font
        $control.Location = New-Object System.Drawing.Point(210, $top - 2)
        $control.Size = New-Object System.Drawing.Size(470, 28)
        $form.Controls.Add($control)
        $controls[$field.Key] = $control
        $top += 54
    }

    $loginCheckbox = New-Object System.Windows.Forms.CheckBox
    $loginCheckbox.Text = "现在执行 wrangler login"
    $loginCheckbox.Font = $font
    $loginCheckbox.Checked = [bool]($null -ne $Defaults["run_wrangler_login"] -and $Defaults["run_wrangler_login"])
    $loginCheckbox.Location = New-Object System.Drawing.Point(210, $top - 2)
    $loginCheckbox.Size = New-Object System.Drawing.Size(220, 28)
    $form.Controls.Add($loginCheckbox)

    $hint = New-Object System.Windows.Forms.Label
    $hint.Text = "说明: public_domain 推荐填写 https://hub.example.com；local_trycloudflare 模式可留空公网地址。"
    $hint.Font = $font
    $hint.ForeColor = [System.Drawing.Color]::DimGray
    $hint.Location = New-Object System.Drawing.Point(20, 470)
    $hint.Size = New-Object System.Drawing.Size(660, 36)
    $form.Controls.Add($hint)

    $okButton = New-Object System.Windows.Forms.Button
    $okButton.Text = "保存授权"
    $okButton.Location = New-Object System.Drawing.Point(480, 505)
    $okButton.Size = New-Object System.Drawing.Size(95, 32)
    $okButton.DialogResult = [System.Windows.Forms.DialogResult]::OK
    $form.Controls.Add($okButton)

    $cancelButton = New-Object System.Windows.Forms.Button
    $cancelButton.Text = "取消"
    $cancelButton.Location = New-Object System.Drawing.Point(585, 505)
    $cancelButton.Size = New-Object System.Drawing.Size(95, 32)
    $cancelButton.DialogResult = [System.Windows.Forms.DialogResult]::Cancel
    $form.Controls.Add($cancelButton)

    $form.AcceptButton = $okButton
    $form.CancelButton = $cancelButton

    if ($form.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) {
        throw "Authorization dialog was cancelled."
    }

    return @{
        default_mode = [string]$controls["default_mode"].SelectedItem
        default_public_url = $controls["default_public_url"].Text
        route_local_part = $controls["route_local_part"].Text
        zone_domain = $controls["zone_domain"].Text
        hub_secret = $controls["hub_secret"].Text
        cloudflare_api_token = $controls["cloudflare_api_token"].Text
        cloudflare_zone_id = $controls["cloudflare_zone_id"].Text
        cloudflare_account_id = $controls["cloudflare_account_id"].Text
        run_wrangler_login = $loginCheckbox.Checked
    }
}

function Normalize-PublicUrl([string]$ModeValue, [string]$CandidateUrl) {
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

function Resolve-WranglerAccountId {
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

Write-Host "========================================================"
Write-Host "PhantomDrop Cloudflare First-Time Authorization"
Write-Host "Project   : $projectRoot"
Write-Host "========================================================"

if ($RunWranglerLogin) {
    Write-Host "[STEP] Running wrangler login" -ForegroundColor Cyan
    Push-Location $networkDir
    try {
        & npx wrangler login
        if ($LASTEXITCODE -ne 0) {
            throw "wrangler login failed."
        }
    } finally {
        Pop-Location
    }
}

$config = Load-Config
$dialogDefaults = @{
    default_mode = Select-Value @($config.default_mode, $DefaultMode)
    default_public_url = Select-Value @($config.default_public_url, $DefaultPublicUrl)
    route_local_part = Select-Value @($config.route_local_part, $RouteLocalPart)
    zone_domain = Select-Value @($config.zone_domain, $ZoneDomain)
    hub_secret = Select-Value @($config.hub_secret, $HubSecret)
    cloudflare_api_token = Select-Value @($config.cloudflare_api_token, $CloudflareApiToken)
    cloudflare_zone_id = Select-Value @($config.cloudflare_zone_id, $CloudflareZoneId)
    cloudflare_account_id = Select-Value @($config.cloudflare_account_id, $CloudflareAccountId)
    run_wrangler_login = [bool]$RunWranglerLogin
}

if (-not $NoDialog) {
    $dialogValues = Show-InitializationDialog -Defaults $dialogDefaults
    $DefaultMode = [string]$dialogValues.default_mode
    $DefaultPublicUrl = [string]$dialogValues.default_public_url
    $RouteLocalPart = [string]$dialogValues.route_local_part
    $ZoneDomain = [string]$dialogValues.zone_domain
    $HubSecret = [string]$dialogValues.hub_secret
    $CloudflareApiToken = [string]$dialogValues.cloudflare_api_token
    $CloudflareZoneId = [string]$dialogValues.cloudflare_zone_id
    $CloudflareAccountId = [string]$dialogValues.cloudflare_account_id
    $RunWranglerLogin = [bool]$dialogValues.run_wrangler_login
}

$config.default_mode = $DefaultMode
$config.hub_secret = if ($null -ne $HubSecret) { $HubSecret.Trim() } else { $HubSecret }
$config.route_local_part = $RouteLocalPart

$normalizedPublicUrl = Normalize-PublicUrl -ModeValue $DefaultMode -CandidateUrl $DefaultPublicUrl
if (-not [string]::IsNullOrWhiteSpace($normalizedPublicUrl)) {
    $config.default_public_url = $normalizedPublicUrl
}

if (-not [string]::IsNullOrWhiteSpace($ZoneDomain)) {
    $config.zone_domain = $ZoneDomain.Trim().ToLowerInvariant()
}
if (-not [string]::IsNullOrWhiteSpace($CloudflareApiToken)) {
    $config.cloudflare_api_token = $CloudflareApiToken.Trim()
}
if (-not [string]::IsNullOrWhiteSpace($CloudflareZoneId)) {
    $config.cloudflare_zone_id = $CloudflareZoneId.Trim()
}
if (-not [string]::IsNullOrWhiteSpace($CloudflareAccountId)) {
    $config.cloudflare_account_id = $CloudflareAccountId.Trim()
}

if (-not $config.ContainsKey("cloudflare_account_id") -or [string]::IsNullOrWhiteSpace([string]$config.cloudflare_account_id)) {
    $resolvedAccountId = Resolve-WranglerAccountId
    if (-not [string]::IsNullOrWhiteSpace($resolvedAccountId)) {
        $config.cloudflare_account_id = $resolvedAccountId
    }
}

Save-Config -Config $config

Write-Host ""
Write-Host "Authorization bootstrap completed." -ForegroundColor Green
$config | ConvertTo-Json -Depth 10
Write-Host ""
Write-Host "Saved to $configPath" -ForegroundColor DarkCyan
