# Release template: publish-homebrew-tap substitutes all RELEASE_* values.
# Do not install this source template directly before a verified release exists.
class Qinaegis < Formula
  desc "Local Web AI quality workbench using Playwright and Midscene.js"
  homepage "https://github.com/mbpz/qinAegis"
  version "RELEASE_VERSION"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mbpz/qinAegis/releases/download/v#{version}/qinaegis-aarch64-apple-darwin.tar.gz"
      sha256 "RELEASE_ARM_SHA256"
    else
      url "https://github.com/mbpz/qinAegis/releases/download/v#{version}/qinaegis-x86_64-apple-darwin.tar.gz"
      sha256 "RELEASE_INTEL_SHA256"
    end
  end

  on_linux do
    url "https://github.com/mbpz/qinAegis/releases/download/v#{version}/qinaegis-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "RELEASE_LINUX_SHA256"
  end

  def install
    bin.install "qinaegis"
  end

  def caveats
    <<~EOS
      Run qinaegis to start the local service and open your browser.
      Run qinaegis stop before upgrading. Project data is preserved.
      Playwright and Midscene.js runtime installation is separate.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/qinaegis version")
  end
end
