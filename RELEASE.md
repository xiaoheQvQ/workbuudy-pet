# WorkBuddy-PET 发布与自动更新流程

本文档记录从「改版本号」到「用户收到更新」的完整流程。下次发版照着走即可。

---

## 0. 关键信息速查

| 项 | 值 |
| --- | --- |
| 仓库 | `https://github.com/xiaoheQvQ/workbuudy-pet` |
| CI 工作流 | `.github/workflows/release.yml` |
| 更新检查地址（写在应用配置里） | `https://github.com/xiaoheQvQ/workbuudy-pet/releases/latest/download/latest.json` |
| 构建平台 | **仅 Windows x64**（NSIS 安装包） |
| 发布模式 | `releaseDraft: false`，CI 完成后**自动发布**，无需人工 Publish |
| 签名私钥 | `.local/workbuddy-updater.key`（已被 `.gitignore` 忽略，**必须异地备份**） |
| 私钥密码 | 见 `.local/SECRETS.local.md`（该目录不提交） |
| 公钥 | 已写入 `src-tauri/tauri.conf.json` → `plugins.updater.pubkey` |
| 安装包命名 | `workbuddy-PET_<版本号>_x64-setup.exe` |

---

## 1. 发版标准流程

### 步骤 1 · 同步三处版本号

版本号必须一致，Tauri 会校验 tag 与版本号是否匹配。三处都要改：

| 文件 | 字段 |
| --- | --- |
| `package.json` | `"version"` |
| `src-tauri/Cargo.toml` | `[package]` 下的 `version` |
| `src-tauri/tauri.conf.json` | `"version"` |

改完用下面的命令确认三处一致（例如都应是 `1.0.5`）：

```powershell
cd d:\A_P\workbuudy-pet
(Get-Content package.json -Raw | ConvertFrom-Json).version
(Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
Select-String -Path src-tauri/Cargo.toml -Pattern '^version' | Select-Object -First 1 -ExpandProperty Line
```

### 步骤 2 · 本地试打包（推荐）

先本地验证能编过，避免 CI 跑一半才失败。

```powershell
cd d:\A_P\workbuudy-pet

# cargo 装在 ~\.cargo\bin，当前 shell 可能没加载，手动补 PATH
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

# 签名相关环境变量（变量名不能写错，Tauri 只认这两个）
$env:TAURI_SIGNING_PRIVATE_KEY = "d:\A_P\workbuudy-pet\.local\workbuddy-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (私钥密码，见 .local/SECRETS.local.md)

pnpm tauri build
```

产物：

```
src-tauri/target/release/bundle/nsis/
├─ workbuddy-PET_<版本号>_x64-setup.exe       ← 安装包
└─ workbuddy-PET_<版本号>_x64-setup.exe.sig   ← 更新签名
```

> 注意：本地 `tauri build` **不会**生成 `latest.json`，那是 CI 里 `tauri-action` 的职责。
> 本地这个包只用于自己装机测试，不要拿去发布。

### 步骤 3 · 提交代码并推 tag（触发 CI）

```powershell
git add -A
git commit -m "release: v1.0.5"
git push

git tag v1.0.5
git push origin v1.0.5
```

tag 名必须是 `v` + 版本号，且与步骤 1 的版本号完全对应。

### 步骤 4 · 等 CI 自动完成

CI 跑完（约 5～10 分钟）会**自动创建并发布** Release，不需要人工点 Publish。
建议仍然核对一下结果：

1. 打开 https://github.com/xiaoheQvQ/workbuudy-pet/actions 确认全绿
2. 打开 https://github.com/xiaoheQvQ/workbuudy-pet/releases 确认该版本已发布，资产应为 3 个：
   - `workbuddy-PET_<版本号>_x64-setup.exe`
   - `workbuddy-PET_<版本号>_x64-setup.exe.sig`
   - `latest.json`

> 发布之后 `releases/latest/download/latest.json` 立即指向它，自动更新随即生效。
> 若某一次想先人工审核再公开，把 `releaseDraft` 临时改回 `true`（见第 5 节）。

### 步骤 5 · 验证自动更新链路

```powershell
Invoke-RestMethod 'https://github.com/xiaoheQvQ/workbuudy-pet/releases/latest/download/latest.json' |
  ConvertTo-Json -Depth 5
```

要确认 `platforms.windows-x86_64` 下的 `url` 和 `signature` 都有值。
`signature` 必须是 `.sig` 文件的**内容**，不能是路径或链接。

---

## 2. CI 具体做了什么

`.github/workflows/release.yml`，推 `v*` tag 或手动触发：

1. `windows-latest` runner 检出代码
2. 装 pnpm 10 / Node 22 / Rust stable，缓存 Cargo 与 pnpm
3. `pnpm install --frozen-lockfile`
4. `tauri-apps/tauri-action` 执行：
   - `pnpm tauri build --bundles nsis`（只出 NSIS，忽略配置里其他 target）
   - 用 Secrets 里的私钥给安装包签名，产出 `.sig`
   - 创建 / 更新对应 tag 的 Release 并直接发布（`releaseDraft: false`）
   - 生成并上传 `latest.json`（自动更新清单）

需要的仓库 Secrets（Settings → Secrets and variables → Actions）：

| Secret 名 | 内容 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | 私钥文件**全文内容** |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码 |

另外需确认 Settings → Actions → General → Workflow permissions 为 **Read and write**，否则 `GITHUB_TOKEN` 建不了 Release。

---

## 3. 自动更新是怎么跑通的

```
应用启动
  └─ src/composables/useAppUpdate.ts 调用 check()
       └─ GET .../releases/latest/download/latest.json
            └─ 用 tauri.conf.json 里的 pubkey 校验 signature
                 └─ 远端版本更高 → 顶部出现「下载更新」
                      └─ 下载 exe → 校验签名 → passive 静默安装 → relaunch()
```

三个前置条件（当前均已就位）：

- `src-tauri/tauri.conf.json`：`bundle.createUpdaterArtifacts: true`、`plugins.updater.pubkey` 与私钥配对、`endpoints` 指向本仓库
- `src-tauri/capabilities/default.json`：含 `updater:default` 与 `process:default`
- Release 上有 `latest.json` 且已 Publish（非草稿、非 prerelease）

> 更新检查失败时 `useAppUpdate.ts` 是静默吞掉的（`.catch(() => null)`），
> 用户只会看到「无更新」。所以改完 endpoint 后务必用步骤 5 的命令实测一次。

---

## 4. 安全须知（重要）

- **私钥 `.local/workbuddy-updater.key` 必须异地备份**（网盘 / 密码管理器都可）。
  丢了以后**已安装的用户将永远无法再收到更新**，唯一出路是换密钥对，
  但老版本客户端里写的是旧公钥，无法验证新包，等于老用户必须手动重装。
- 私钥、私钥密码**都不要提交到仓库**。`.local/` 已在 `.gitignore` 里，别往里塞会被提交的东西。
- 公钥（`pubkey`）是公开信息，写进配置没问题。
- 更换密钥对时：`pnpm tauri signer generate` 生成新对 → 新公钥写进 `tauri.conf.json` →
  新私钥更新到 GitHub Secrets。

---

## 5. 常见问题

### 本地打包卡在下载 NSIS 工具链（`timeout: global`）

Tauri 不读 Windows 系统代理，走代理也无效，需要手工把工具链放进缓存目录。

缓存根目录：`%LOCALAPPDATA%\tauri\`，NSIS 期望结构：

```
%LOCALAPPDATA%\tauri\NSIS\
├─ makensis.exe
├─ Bin\makensis.exe
├─ Stubs\lzma-x86-unicode
├─ Stubs\lzma_solid-x86-unicode
└─ Plugins\x86-unicode\additional\nsis_tauri_utils.dll
```

手工下载（浏览器能访问 GitHub 时直接下，或走代理用 `Invoke-WebRequest`，**不要用 curl**）：

| 文件 | 来源 | SHA1 |
| --- | --- | --- |
| `nsis-3.11.zip` | `https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip` | `EF7FF767E5CBD9EDD22ADD3A32C9B8F4500BB10D` |
| `nsis_tauri_utils.dll` | `https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll` | `75197FEE3C6A814FE035788D1C34EAD39349B860` |

解压 `nsis-3.11.zip` 后里面还套了一层 `nsis-3.11/`，**要把内层内容提升到 `NSIS\` 根下**，
否则 Tauri 找不到 `makensis.exe` 会重下。

### `curl` 下载 GitHub 超时，但浏览器/`Invoke-WebRequest` 正常

`curl.exe` 不读取 Windows 的系统代理设置，而 `Invoke-WebRequest` 会读。
本机系统代理为 `127.0.0.1:7890`，所以下载统一用 `Invoke-WebRequest`。

### CI 报 `A public key has been found, but no private key`

Secrets 名字写错了，或没配。必须是 `TAURI_SIGNING_PRIVATE_KEY`（不是 `..._PATH` 变体）。

### CI 报 tag 与版本号不匹配

`git tag` 的版本号与 `tauri.conf.json` 的 `version` 不一致，两者必须相同。

### 想改成「先人工审核再公开」的草稿模式

把 `.github/workflows/release.yml` 里的 `releaseDraft: false` 改成 `true`。
CI 会产出草稿 Release，需在 Releases 页面手动点 Publish 才对用户生效。

### 以后想加 macOS / Linux

1. 在 `jobs.publish` 上加回 `strategy.matrix`，并补 Linux 系统依赖安装步骤
2. 移除 `tauri-action` 的 `args: '--bundles nsis'`
3. macOS 需注意 `tauri.conf.json` 里的 `macOSPrivateApi: true` 与 `signingIdentity: "-"`（ad-hoc 签名）

---

## 6. 首次在本机准备构建环境

新机器从零开始时执行：

```powershell
pwsh -File .\scripts\setup-rust-env.ps1 -UseMirror
```

该脚本会装 MSVC 生成工具 + rustup + stable-msvc 工具链，并对 `src-tauri` 跑一次 `cargo check` 验证。
