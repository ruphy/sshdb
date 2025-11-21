class Sshdb < Formula
  desc "Keyboard-first SSH library and launcher TUI"
  homepage "https://github.com/ruphy/sshdb"
  url "https://github.com/ruphy/sshdb/archive/refs/tags/v0.15.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_SHA256"
  license "GPL-3.0-only"
  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/sshdb", "--help"
  end
end

