class Warden < Formula
  desc "Runtime control layer for AI coding agents"
  homepage "https://github.com/ekud12/warden"
  version "2.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ekud12/warden/releases/download/v2.0.0/warden-aarch64-apple-darwin"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/ekud12/warden/releases/download/v2.0.0/warden-x86_64-apple-darwin"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    url "https://github.com/ekud12/warden/releases/download/v2.0.0/warden-x86_64-unknown-linux-gnu"
    sha256 "PLACEHOLDER"
  end

  def install
    bin.install "warden-*" => "warden"
  end

  def post_install
    system bin/"warden", "version"
  end

  test do
    assert_match "warden", shell_output("#{bin}/warden version")
  end
end
