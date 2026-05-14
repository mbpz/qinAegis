# QinAegis CI/CD 集成

> 本文档介绍 GitHub Actions 集成方式

## 发布：Homebrew Cask

文件：`.github/workflows/publish-homebrew-tap.yml`

### 前提条件

1. GitHub Release 已发布（tag `v*`）
2. Release 产物包含两个 DMG：
   - `QinAegis-{version}-mac-arm64.dmg`
   - `QinAegis-{version}-mac-x64.dmg`
3. `homebrew-tap` 仓库（`mbpz/homebrew-tap`）已创建

### 工作流程

1. 下载两个架构的 DMG
2. 计算 SHA256
3. 生成 `qinaegis.rb` cask 文件
4. Push 到 `homebrew-tap` 仓库

### 安装命令

```bash
brew install --cask mbpz/jinjiao/jinjiao
```

---