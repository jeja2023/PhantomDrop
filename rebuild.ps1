# 幻影中枢 一键清理并重新编译脚本
chcp 65001 > $null
Write-Host '--- Clean and Build ---' -ForegroundColor Cyan
taskkill /F /IM core.exe /T 2>$null
Set-Location "$PSScriptRoot\core"
cargo build
if ($LASTEXITCODE -eq 0) {
    Write-Host 'Success!' -ForegroundColor Green
}
else {
    Write-Host 'Error!' -ForegroundColor Red
    exit 1
}
