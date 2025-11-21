class Sshdb < Formula
  desc "Keyboard-first SSH library and launcher TUI"
  homepage "https://github.com/ruphy/sshdb"
  url "https://github.com/ruphy/sshdb/archive/refs/tags/v0.15.0.tar.gz"
  sha256 "f0fed6beb31bc95fd75b7ed9e1dd0cd11a5588e3934b27d8b469049c91a27e57"
  license "GPL-3.0-or-later"
  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/sshdb", "--help"
  end
end

