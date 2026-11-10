cask "quacko" do
  version "0.1.0"
  sha256 :no_check

  url "https://github.com/larasouzabr/quacko/releases/download/v#{version}/Quacko_universal.dmg"
  name "Quacko"
  desc "Pixel-art duck that tells you when Claude Code, Cursor or CI finish"
  homepage "https://github.com/larasouzabr/quacko"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :big_sur"

  app "Quacko.app"

  uninstall quit: "dev.quacko.notifier"

  zap trash: [
    "~/.cache/quacko",
    "~/.config/quacko",
    "~/Library/Caches/dev.quacko.notifier",
    "~/Library/LaunchAgents/Quacko.plist",
    "~/Library/WebKit/dev.quacko.notifier",
  ]

  caveats <<~EOS
    Quacko isn't signed by Apple yet. The first time you open it, macOS blocks it:
    go to System Settings → Privacy & Security and click "Open Anyway".
  EOS
end
