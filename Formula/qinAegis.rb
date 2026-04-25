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
  depends_on "docker" => :recommended

  def install
    bin.install "qinAegis"

    # Install sandbox docker-compose template
    (etc/"qinAegis").install "docker/docker-compose.sandbox.yml"
  end

  def post_install
    (var/"log/qinAegis").mkpath
  end

  def caveats
    <<~EOS
      To get started:
        qinAegis init

      Docker is required for sandbox execution:
        brew install --cask docker

      For full documentation:
        https://github.com/yourorg/qinAegis
    EOS
  end

  test do
    system "#{bin}/qinAegis", "--version"
  end
end