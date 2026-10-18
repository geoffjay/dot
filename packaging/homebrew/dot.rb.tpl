# typed: false
# frozen_string_literal: true

# Homebrew formula for dot.
#
# This file is the source of truth for the formula published to the
# geoffjay/homebrew-tap tap. It is regenerated for each release by
# scripts/gen-homebrew-formula.sh, which fills in the version and the
# per-target sha256 checksums from the release's checksums.txt.
class Dot < Formula
  desc "Modular, versioned dotfile manager"
  homepage "https://github.com/geoffjay/dot"
  version "0.0.0"
  license "MIT OR Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/geoffjay/dot/releases/download/v#{version}/dot-aarch64-apple-darwin.tar.gz"
      sha256 "SHA256_AARCH64_APPLE_DARWIN"
    end
    on_intel do
      url "https://github.com/geoffjay/dot/releases/download/v#{version}/dot-x86_64-apple-darwin.tar.gz"
      sha256 "SHA256_X86_64_APPLE_DARWIN"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/geoffjay/dot/releases/download/v#{version}/dot-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_AARCH64_UNKNOWN_LINUX_GNU"
    end
    on_intel do
      url "https://github.com/geoffjay/dot/releases/download/v#{version}/dot-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_X86_64_UNKNOWN_LINUX_GNU"
    end
  end

  def install
    bin.install "dot"
  end

  test do
    assert_match "dot", shell_output("#{bin}/dot --version")
  end
end