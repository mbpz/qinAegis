# qinAegis 安装指南

## 前置要求

- macOS 12.0 或更高版本
- Node.js 18+（用于沙箱层）
- Homebrew（用于 Homebrew 安装方式）

## 通过 Homebrew 安装（推荐）

### 方式一：直接安装

```bash
brew install mbpz/qinAegis/qinAegis
```

### 方式二：先添加 tap

```bash
brew tap mbpz/qinAegis
brew install qinAegis
```

安装完成后，Playwright 浏览器会在首次运行时自动安装。

## 从源码安装

```bash
# 克隆项目
git clone https://github.com/mbpz/qinAegis.git
cd qinAegis

# 构建 Release 版本
cargo build --release

# 安装到 PATH
cargo install --path crates/cli

# 或直接使用构建好的二进制
./target/release/qinAegis --help
```

## 安装 Playwright 浏览器

```bash
# 如果使用 Homebrew 安装，Playwright 会在首次运行时自动下载
# 手动安装（可选）：
cd sandbox
pnpm install
pnpm exec playwright install chromium
```

## 安装后配置

```bash
# 初始化配置（引导设置 AI 模型等）
qinAegis init

# 添加项目
qinAegis project add --name my-app --url http://localhost:3000

# 探索项目
qinAegis explore --project my-app
```

## 常见问题

### Q: 安装失败怎么办？

1. 确认 Homebrew 已正确安装
2. 确认 macOS 版本 >= 12.0
3. 尝试手动安装 Playwright：`pnpm exec playwright install chromium`

### Q: Playwright 下载慢？

可以使用国内镜像：
```bash
PLAYWRIGHT_DOWNLOAD_HOST=https://npmmirror.com npx playwright install chromium
```

### Q: 需要 Docker 吗？

**不需要**。qinAegis 使用 Playwright 直接管理浏览器实例，无需 Docker 或任何容器运行时。

## 更多信息

- 用户手册：[docs/USER_GUIDE.md](./docs/USER_GUIDE.md)
- 完整文档：[README_zh.md](./README_zh.md)