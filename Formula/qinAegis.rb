# Formula/qinAegis.rb
class QinAegis < Formula
  desc "AI-powered automated testing TUI for web projects"
  homepage "https://github.com/yourorg/qinAegis"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yourorg/qinAegis/releases/download/v0.1.0/qinAegis-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ARM_SHA256"
    else
      url "https://github.com/yourorg/qinAegis/releases/download/v0.1.0/qinAegis-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_SHA256"
    end
  end

  depends_on :macos

  def install
    bin.install "qinAegis"
  end

  def post_install
    (var/"log/qinAegis").mkpath
  end

  def caveats
    <<~EOS
      快速开始：
        qinAegis init

      Playwright 浏览器会在首次运行时自动安装。
      无需 Docker 或容器运行时。

      完整文档：
        https://github.com/yourorg/qinAegis/blob/main/README_zh.md
    EOS
  end

  test do
    system "#{bin}/qinAegis", "--version"
  end
end