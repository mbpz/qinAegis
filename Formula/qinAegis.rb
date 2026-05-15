cask "qinaegis" do
  arch arm: "arm64", intel: "x64"

  version "0.1.0"
  sha256 arm:   "REPLACE_WITH_ARM_SHA256",
         intel: "REPLACE_WITH_X86_SHA256"

  url "https://github.com/mbpz/qinAegis/releases/download/v#{version}/QinAegis-v#{version}-mac-#{arch}.dmg"
  name "QinAegis"
  desc "AI-powered quality engineering platform for Web applications"
  homepage "https://github.com/mbpz/qinAegis"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: ">= :catalina"

  artifact "QinAegis.app", target: "/Applications/QinAegis.app"

  uninstall quit: "com.qinaegis.app",
            delete: "/Applications/QinAegis.app"

  zap trash: [
    "~/.qinAegis",
    "~/Library/Application Support/com.qinaegis.app",
    "~/Library/Preferences/com.qinaegis.app.plist",
    "~/Library/Saved Application State/com.qinaegis.app.savedState",
  ]

  caveats <<~EOS
    After installation, find QinAegis in your Applications folder.
    Double-click to launch the GUI application.
  EOS
end
