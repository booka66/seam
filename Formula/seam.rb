# Copy to booka66/homebrew-seam at a release, with the tag's url and sha256.
class Seam < Formula
  desc "A change, read by definition rather than by file"
  homepage "https://github.com/booka66/seam"
  url "https://github.com/booka66/seam/archive/refs/tags/vVERSION.tar.gz"
  sha256 "SHA256"

  depends_on "rust" => :build
  depends_on "jq"

  def install
    system "cargo", "build", "--release", "--locked"
    # seam finds seam-split beside itself, and seam-split finds share/ beside its bin/.
    libexec.install "share"
    (libexec/"bin").install "target/release/seam", "bin/seam-split"
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
