# 安装

## 通过 Homebrew（推荐）

```bash
brew install --cask mbpz/qinAegis/qinAegis
```

这将：
1. 下载适合您架构的最新 DMG（arm64 或 x86_64）
2. 挂载 DMG 并将 QinAegis.app 复制到 `/Applications`
3. 移除隔离属性，避免 Gatekeeper 拦截

安装完成后，在应用程序文件夹中找到 **QinAegis.app**。

## 从 DMG（手动安装）

1. 从 GitHub Releases 下载 DMG
2. 双击挂载
3. 将 QinAegis.app 拖到应用程序文件夹
4. （首次运行）在 Gatekeeper 对话框中点击"打开"

## 从源码构建

```bash
git clone https://github.com/mbpz/qinAegis.git
cd qinAegis

# 构建 React UI
cd crates/web_client/ui && npm install && npm run build && cd ../..

# 构建 Rust 二进制
cargo build --release --bin qinAegis-web
```

二进制文件位于 `target/release/qinAegis-web`。

## 系统要求

- macOS 10.15 或更高版本
- Apple Silicon（M1/M2/M3）或 Intel Mac

## 首次启动

1. 在应用程序文件夹中双击 QinAegis.app
2. 如果看到 Gatekeeper 警告，在系统偏好设置 > 安全性中点击"仍然打开"
3. GUI 将打开，并显示 AI 模型配置的设置向导

## 卸载

```bash
rm -rf /Applications/QinAegis.app
# 可选：移除配置（您的测试数据将保留）
rm -rf ~/.config/qinAegis
```