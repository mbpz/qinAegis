# qinAegis 安装指南

## 通过 Homebrew 安装（推荐）

```bash
brew install yourorg/qinAegis/qinAegis
```

或先添加 tap：

```bash
brew tap yourorg/qinAegis
brew install qinAegis
```

## 从源码安装

```bash
git clone https://github.com/yourorg/qinAegis.git
cd qinAegis
cargo install --path crates/cli
```

## 系统要求

- macOS 12.0 或更高版本
- Node.js 18+（用于沙箱层）
- Playwright（自动管理，无需手动安装）

```bash
# Playwright 浏览器在首次运行时自动安装
# 如需手动安装：
cd sandbox && pnpm exec playwright install chromium
```

## 安装后配置

安装完成后，运行初始化命令：

```bash
qinAegis init
```

这将引导你完成：
1. AI 模型配置
2. 配置目录设置
3. Playwright 浏览器安装（仅首次）

## 无 Docker 要求

qinAegis 使用 Playwright 直接管理浏览器实例，无需 Docker 或容器运行时。