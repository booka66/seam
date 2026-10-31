# Copy to booka66/homebrew-seam at a release, with the version and each tarball's
# sha256 from the tag's GitHub release. It installs the build CI made, so nothing
# here wants a compiler.
class Seam < Formula
  desc "A change, read by definition rather than by file"
  homepage "https://github.com/booka66/seam"
  version "VERSION"

  base = "https://github.com/booka66/seam/releases/download/v#{version}/seam-#{version}"
  on_macos do
    on_arm do
      url "#{base}-aarch64-apple-darwin.tar.gz"
      sha256 "SHA_AARCH64_APPLE_DARWIN"
    end
    on_intel do
      url "#{base}-x86_64-apple-darwin.tar.gz"
      sha256 "SHA_X86_64_APPLE_DARWIN"
    end
  end
  on_linux do
    on_arm do
      url "#{base}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "SHA_AARCH64_UNKNOWN_LINUX_MUSL"
    end
    on_intel do
      url "#{base}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "SHA_X86_64_UNKNOWN_LINUX_MUSL"
    end
  end

  depends_on "jq"

  def install
    # seam finds seam-split beside itself, and seam-split finds share/ beside its bin/.
    libexec.install "bin", "share"
    bin.write_exec_script libexec/"bin/seam"
  end

  test do
    system "git", "init", "-q", "."
    system "git", "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "base"
    (testpath/"a.ts").write "export function run() { return 1; }\n"
    system "git", "add", "a.ts"
    system "git", "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "head"
    assert_match "run", shell_output("#{bin}/seam HEAD")
    assert_match "slices", shell_output("#{bin}/seam HEAD --split")
  end
end
