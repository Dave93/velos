class Velos < Formula
  desc "High-performance AI-friendly process manager"
  homepage "https://github.com/user/velos"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/user/velos/releases/download/v#{version}/velos-macos-arm64"
      sha256 "PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/user/velos/releases/download/v#{version}/velos-macos-x86_64"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/user/velos/releases/download/v#{version}/velos-linux-arm64"
      sha256 "PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/user/velos/releases/download/v#{version}/velos-linux-x86_64"
      sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install Dir["velos-*"].first => "velos"
  end

  test do
    assert_match "velos", shell_output("#{bin}/velos --version")
  end
end
