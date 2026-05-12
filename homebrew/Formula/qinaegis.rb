cask "qinaegis" do
  arch arm: "arm64", intel: "x64"

  version "0.1.0"
  sha256 arm:   "REPLACE_WITH_ARM64_SHA256",
         intel: "REPLACE_WITH_X86_SHA256"

  url "https://github.com/qinaegis/qinAegis/releases/download/#{version}/QinAegis-#{version}-mac-#{arch}.dmg"
  name "QinAegis"
  desc "AI-powered terminal productivity tool"
  homepage "https://github.com/qinaegis/qinAegis"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates false
  depends_on macos: ">= :catalina"

  app "QinAegis.app"

  zap trash: [
    "~/Library/Application Support/com.qinaegis.qinAegis",
    "~/Library/Preferences/com.qinaegis.qinAegis.plist",
    "~/Library/Saved Application State/com.qinaegis.qinAegis.savedState",
  ]
end