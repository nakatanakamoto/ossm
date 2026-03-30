param(
    [Parameter(Mandatory=$true)]
    [string]$Crate
)

if (-not (Test-Path "firmware/$Crate/Cargo.toml")) {
    Write-Error "firmware/$Crate/Cargo.toml does not exist"
    exit 1
}

$vscode = Get-Content ".vscode/settings.template.json" | ConvertFrom-Json
$vscode | Add-Member -Force "rust-analyzer.linkedProjects" @("firmware/$Crate/Cargo.toml")
$vscode | ConvertTo-Json -Depth 10 | Set-Content ".vscode/settings.json"

$zed = Get-Content ".zed/settings.template.json" | ConvertFrom-Json
$zed.lsp.'rust-analyzer'.initialization_options | Add-Member -Force "linkedProjects" @("firmware/$Crate/Cargo.toml")
$zed | ConvertTo-Json -Depth 10 | Set-Content ".zed/settings.json"

Write-Host "rust-analyzer focused on $Crate"
