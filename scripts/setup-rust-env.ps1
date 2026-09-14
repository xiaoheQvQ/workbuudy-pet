#Requires -Version 5.1
<#
.SYNOPSIS
    为 WorkBuddy-PET（Tauri 2）在 Windows 上配置完整的 Rust 构建环境。

.DESCRIPTION
    幂等脚本，可重复执行。依次完成：
      1. 检测并安装 Microsoft C++ 生成工具（VS 2022 Build Tools + VCTools 工作负载 + Windows SDK）
      2. 检测并安装 rustup，配置默认工具链 stable-x86_64-pc-windows-msvc
      3. 补齐 rustfmt / clippy 组件
      4. 可选：写入全局 ~/.cargo/config.toml（crates.io 镜像 + git-fetch-with-cli）
      5. 校验 cargo / rustc / link.exe，并对 src-tauri 执行 cargo check 编译验证

    说明：本项目的依赖集（ring 0.17 预生成汇编、rusqlite bundled、无 aws-lc-rs）
    在 Windows MSVC 下不需要 NASM / CMake / Perl。

.PARAMETER UseMirror
    使用 rsproxy.cn 稀疏索引镜像替换 crates.io（国内网络建议开启）。

.PARAMETER SkipBuildTools
    跳过 MSVC C++ 生成工具安装（已装完整版 Visual Studio 时使用）。

.PARAMETER SkipCheck
    跳过最后的 cargo check 编译验证。

.EXAMPLE
    pwsh -File .\scripts\setup-rust-env.ps1 -UseMirror -Verbose

.EXAMPLE
    # 只做环境校验，不安装任何东西
    pwsh -File .\scripts\setup-rust-env.ps1 -SkipBuildTools -SkipCheck
#>
[CmdletBinding()]
param(
    [switch]$UseMirror,
    [switch]$SkipBuildTools,
    [switch]$SkipCheck
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot  = Split-Path -Parent $PSScriptRoot
$TauriDir  = Join-Path $RepoRoot 'src-tauri'
$CargoHome = Join-Path $env:USERPROFILE '.cargo'
$CargoBin  = Join-Path $CargoHome 'bin'
$CargoExe  = Join-Path $CargoBin 'cargo.exe'
$RustupExe = Join-Path $CargoBin 'rustup.exe'
$ConfigToml = Join-Path $CargoHome 'config.toml'
$Vswhere   = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'

# MSVC 工作负载：Desktop development with C++（含 x64 工具集与 Windows SDK）
$VsWorkloadArgs = @(
    '--quiet', '--wait', '--norestart'
    '--add', 'Microsoft.VisualStudio.Workload.VCTools'
    '--includeRecommended'
) -join ' '

function Write-Step { param([string]$Message) Write-Host "`n==> $Message" -ForegroundColor Cyan }
function Write-Ok   { param([string]$Message) Write-Host "  [OK]   $Message" -ForegroundColor Green }
function Write-Warn { param([string]$Message) Write-Host "  [WARN] $Message" -ForegroundColor Yellow }
function Write-Info { param([string]$Message) Write-Host "  [INFO] $Message" -ForegroundColor Gray }

function Test-Cmd { param([string]$Name) [bool](Get-Command $Name -ErrorAction SilentlyContinue) }

function Update-ProcessPath {
    $extra = @(
        $CargoBin
        (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer')
    )
    foreach ($p in $extra) {
        if ((Test-Path $p) -and ($env:Path -notlike "*$p*")) { $env:Path = "$p;$env:Path" }
    }
}
Update-ProcessPath

# ---------------------------------------------------------------- MSVC 生成工具

function Get-MsvcInstallPath {
    if (-not (Test-Path $Vswhere)) { return $null }
    $path = & $Vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath 2>$null
    if ([string]::IsNullOrWhiteSpace($path)) { return $null }
    return $path.Trim()
}

function Install-BuildTools {
    if ($SkipBuildTools) { Write-Info '按参数跳过 MSVC 生成工具安装'; return }
    $existing = Get-MsvcInstallPath
    if ($existing) { Write-Ok "MSVC C++ 工具集已就绪：$existing"; return }

    if (-not (Test-Cmd winget)) {
        Write-Warn 'winget 不可用，请手动安装：https://visualstudio.microsoft.com/visual-cpp-build-tools/'
        Write-Warn '安装时勾选「使用 C++ 的桌面开发」工作负载，然后重新运行本脚本。'
        return
    }

    Write-Step '安装 Microsoft C++ 生成工具（约 2-3 GB，会弹出 UAC，请确认）'
    winget install --id Microsoft.VisualStudio.2022.BuildTools -e `
        --accept-package-agreements --accept-source-agreements `
        --disable-interactivity --override $VsWorkloadArgs
    if ($LASTEXITCODE -ne 0) { Write-Warn "winget 退出码 $LASTEXITCODE，请检查上方输出" }

    Update-ProcessPath
    $installed = Get-MsvcInstallPath
    if ($installed) { Write-Ok "MSVC C++ 工具集安装完成：$installed" }
    else { Write-Warn '未检测到 VC 工具集，可能需重启后重新运行本脚本' }
}

# ---------------------------------------------------------------------- rustup

function Install-Rustup {
    if (Test-Path $RustupExe) { Write-Ok "rustup 已安装：$(& $RustupExe --version)"; return }

    Write-Step '安装 rustup'
    if (Test-Cmd winget) {
        winget install --id Rustlang.Rustup -e `
            --accept-package-agreements --accept-source-agreements --disable-interactivity
        Update-ProcessPath
    }

    if (-not (Test-Path $RustupExe)) {
        Write-Info 'winget 未装成功，回退到官方 rustup-init.exe'
        $init = Join-Path $env:TEMP 'rustup-init.exe'
        $url  = 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
        Invoke-WebRequest -Uri $url -OutFile $init -UseBasicParsing
        & $init -y --no-modify-path --default-toolchain stable-x86_64-pc-windows-msvc --profile default
        if ($LASTEXITCODE -ne 0) { throw 'rustup-init 安装失败' }
        Update-ProcessPath
    }

    if (-not (Test-Path $RustupExe)) { throw 'rustup 安装后仍未找到，请重启终端后重试' }
    Write-Ok "rustup 就绪：$(& $RustupExe --version)"
}

function Set-Toolchain {
    Write-Step '配置默认工具链 stable-x86_64-pc-windows-msvc'
    & $RustupExe toolchain install stable-x86_64-pc-windows-msvc --profile default --no-self-update
    if ($LASTEXITCODE -ne 0) { throw '工具链安装失败' }
    & $RustupExe default stable-x86_64-pc-windows-msvc
    & $RustupExe component add rustfmt clippy
    Write-Ok "$(& $RustupExe show)"
}

# ------------------------------------------------------------ 全局 cargo 配置

function Set-CargoConfig {
    Write-Step "写入全局 cargo 配置：$ConfigToml"
    New-Item -ItemType Directory -Force -Path $CargoHome | Out-Null

    if ((Test-Path $ConfigToml) -and `
        ((Get-Content -Raw $ConfigToml) -notmatch 'setup-rust-env\.ps1')) {
        $backup = "$ConfigToml.bak"
        Copy-Item $ConfigToml $backup -Force
        Write-Warn "已备份原配置到 $backup"
    }

    $lines = @('# 由 scripts/setup-rust-env.ps1 生成')
    if ($UseMirror) {
        $lines += @(
            '# crates.io 走 rsproxy 稀疏索引（国内加速）'
            '[source.crates-io]'
            "replace-with = 'rsproxy-sparse'"
            ''
            '[source.rsproxy-sparse]'
            "registry = 'sparse+https://rsproxy.cn/index/'"
            ''
        )
    }
    $lines += @(
        '[net]'
        '# [patch.crates-io] 中的 git 依赖（wry fork）改用系统 git 拉取，便于走系统代理'
        'git-fetch-with-cli = true'
    )

    $lines | Set-Content -Path $ConfigToml -Encoding UTF8

    if ($UseMirror) { Write-Ok '全局 cargo 配置写入完成（已启用 rsproxy 镜像）' }
    else { Write-Ok '全局 cargo 配置写入完成（未启用镜像，可加 -UseMirror 加速）' }
}

# ---------------------------------------------------------------------- 校验

function Test-Environment {
    Write-Step '校验工具链'
    $ok = $true
    foreach ($tool in @(@('rustc', $null), @('cargo', $null))) {
        $name = $tool[0]
        if (Test-Cmd $name) { Write-Ok "$name $((& $name --version) -join ' ')" }
        else { Write-Warn "$name 不在 PATH 中，请重启终端"; $ok = $false }
    }

    $msvc = Get-MsvcInstallPath
    if ($msvc) { Write-Ok 'MSVC link.exe 可用（rustc 会自动定位）' }
    else { Write-Warn '未找到 MSVC 生成工具，编译将失败于链接阶段' ; $ok = $false }

    return $ok
}

function Invoke-CargoCheck {
    if ($SkipCheck) { Write-Info '按参数跳过 cargo check'; return }

    Write-Step '编译验证：cargo check（src-tauri）'
    Push-Location $TauriDir
    try {
        & $CargoExe check
        if ($LASTEXITCODE -eq 0) { Write-Ok '后端编译验证通过' }
        else { Write-Warn "cargo check 失败，退出码 $LASTEXITCODE（请查看上方错误）" }
    }
    finally { Pop-Location }
}

# ---------------------------------------------------------------------- main

Write-Host 'WorkBuddy-PET —— Rust / Tauri 构建环境配置' -ForegroundColor Magenta
Write-Info "仓库根目录：$RepoRoot"

Install-BuildTools
Install-Rustup
Set-Toolchain
Set-CargoConfig
$envOK = Test-Environment

if ($envOK) { Invoke-CargoCheck }
else { Write-Warn '环境不完整，已跳过 cargo check；请修复上述问题后重新运行' }

Write-Host "`n配置流程结束。若 PATH 未刷新，请重开终端后执行：" -ForegroundColor Magenta
Write-Host "  cd $TauriDir; cargo check" -ForegroundColor Magenta
