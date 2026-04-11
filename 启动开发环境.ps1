param(
    [ValidateSet("web", "console")]
    [string]$Mode = "web",
    [string]$HubSecret = "local_dev_secret",
    [switch]$SeparateWindows
)

chcp 65001 > $null
$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8

$projectRoot = $PSScriptRoot
$coreDir = Join-Path $projectRoot "core"
$webDir = Join-Path $projectRoot "web"
$powershellExe = Join-Path $env:WINDIR "System32\WindowsPowerShell\v1.0\powershell.exe"
$dbUrl = "sqlite://phantom_core.db?mode=rwc"
$coreStdoutLog = Join-Path $coreDir "dev-core-runtime.log"
$coreStderrLog = Join-Path $coreDir "dev-core-runtime.error.log"
$webStdoutLog = Join-Path $webDir "dev-web-runtime.log"
$webStderrLog = Join-Path $webDir "dev-web-runtime.error.log"

if (-not (Test-Path $coreDir)) {
    Write-Host "错误：缺少 core 目录。" -ForegroundColor Red
    exit 1
}

if ($Mode -eq "web" -and -not (Test-Path $webDir)) {
    Write-Host "错误：缺少 web 目录。" -ForegroundColor Red
    exit 1
}

function Start-PhantomProcess {
    param(
        [string]$WorkingDir,
        [string]$Command,
        [string]$StdoutLog,
        [string]$StderrLog,
        [switch]$Visible
    )

    $escapedWorkingDir = $WorkingDir.Replace("'", "''")
    $script = @"
Set-Location '$escapedWorkingDir'
$Command
"@

    $arguments = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass"
    )

    if ($Visible) {
        $arguments += @(
            "-NoExit",
            "-Command",
            $script
        )

        return Start-Process -FilePath $powershellExe -ArgumentList $arguments -WindowStyle Normal -PassThru
    }

    New-Item -ItemType File -Force -Path $StdoutLog | Out-Null
    New-Item -ItemType File -Force -Path $StderrLog | Out-Null

    $arguments += @(
        "-Command",
        $script
    )

    return Start-Process -FilePath $powershellExe -ArgumentList $arguments -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput $StdoutLog `
        -RedirectStandardError $StderrLog
}

function Get-ListeningProcessId {
    param(
        [int]$Port
    )

    $lines = netstat -ano | Select-String -Pattern @(
        "127.0.0.1:$Port",
        "0.0.0.0:$Port",
        "\[::1\]:$Port",
        "\[::\]:$Port"
    )

    if (-not $lines) {
        return $null
    }

    $listening = $lines | Where-Object { $_.ToString() -match "LISTENING" } | Select-Object -First 1
    if (-not $listening) {
        return $null
    }

    $parts = ($listening.ToString() -split "\s+") | Where-Object { $_ }
    return [int]$parts[-1]
}

Write-Host "========================================================"
Write-Host "幻影中枢开发环境启动器" -ForegroundColor Cyan
Write-Host "启动模式  : $Mode" -ForegroundColor DarkCyan
Write-Host "项目目录  : $projectRoot" -ForegroundColor DarkCyan
Write-Host "授权密钥  : $HubSecret" -ForegroundColor DarkCyan
Write-Host "数据库地址: $dbUrl" -ForegroundColor DarkCyan
$winMode = if ($SeparateWindows) { "Multi-Window" } else { "Single-Window" }
if ($winMode -eq "Multi-Window") { Write-Host "窗口模式  : 多窗口" -ForegroundColor DarkCyan } else { Write-Host "窗口模式  : 单窗口 / 后台" -ForegroundColor DarkCyan }
Write-Host "========================================================"

if ($Mode -eq "console") {
    Write-Host "正在启动 Rust 后端与内建控制台..." -ForegroundColor Yellow
    Start-Sleep -Seconds 1
    Start-Process "http://127.0.0.1:9010/"
    Set-Location $coreDir
    $env:HUB_SECRET = $HubSecret
    $env:PHANTOM_DB_URL = $dbUrl
    cargo run
    exit 0
}

$corePid = Get-ListeningProcessId -Port 9010
if ($corePid) {
    Write-Host "检测到后端已在运行，复用进程：$corePid" -ForegroundColor Green
} else {
    Write-Host "正在启动 Rust 后端..." -ForegroundColor Yellow
    $coreProcess = Start-PhantomProcess -WorkingDir $coreDir -StdoutLog $coreStdoutLog -StderrLog $coreStderrLog -Visible:$SeparateWindows -Command @"
`$env:HUB_SECRET = '$HubSecret'
`$env:PHANTOM_DB_URL = '$dbUrl'
cargo run
"@

    if ($SeparateWindows) {
        Write-Host "后端已在独立窗口启动。" -ForegroundColor DarkYellow
    } else {
        Write-Host "后端已在后台启动，进程号：$($coreProcess.Id)" -ForegroundColor DarkYellow
        Write-Host "后端日志：$coreStdoutLog" -ForegroundColor DarkGray
        Write-Host "错误日志：$coreStderrLog" -ForegroundColor DarkGray
    }
}

$webPid = Get-ListeningProcessId -Port 5173
if ($webPid) {
    Write-Host "检测到前端已在运行，复用进程：$webPid" -ForegroundColor Green
} else {
    Write-Host "正在启动前端开发服务..." -ForegroundColor Yellow
    $webProcess = Start-PhantomProcess -WorkingDir $webDir -StdoutLog $webStdoutLog -StderrLog $webStderrLog -Visible:$SeparateWindows -Command @"
`$env:VITE_BACKEND_URL = 'http://127.0.0.1:9010'
npm run dev
"@

    if ($SeparateWindows) {
        Write-Host "前端已在独立窗口启动。" -ForegroundColor DarkYellow
    } else {
        Write-Host "前端已在后台启动，进程号：$($webProcess.Id)" -ForegroundColor DarkYellow
        Write-Host "前端日志：$webStdoutLog" -ForegroundColor DarkGray
        Write-Host "错误日志：$webStderrLog" -ForegroundColor DarkGray
    }
}

Start-Sleep -Seconds 2
Write-Host "后端地址： http://127.0.0.1:9010/" -ForegroundColor Green
Write-Host "前端地址： http://127.0.0.1:5173/" -ForegroundColor Green
Write-Host "正在打开前端页面..." -ForegroundColor Yellow
Start-Process "http://127.0.0.1:5173/"
