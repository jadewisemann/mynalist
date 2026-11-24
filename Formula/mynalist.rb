# Homebrew formula for the Mynalist sync server (apps/server) and the web app it serves (apps/web).
#
#   brew tap jadewisemann/mynalist https://github.com/jadewisemann/mynalist
#   brew install --HEAD mynalist
#   mynalist init && brew services start mynalist
#
# This repository is its own tap (Homebrew reads Formula/ from it), so there is no separate
# homebrew-* repository. Until a release is tagged the formula builds from main (--HEAD);
# deploy/homebrew/bump.sh <tag> adds the stable url/sha256 for a tag.
class Mynalist < Formula
  desc "Self-hosted keyboard-first outliner: sync server and web app"
  homepage "https://github.com/jadewisemann/mynalist"
  head "https://github.com/jadewisemann/mynalist.git", branch: "main"

  depends_on "node"

  def install
    system "npm", "ci", "--no-audit", "--no-fund", "--ignore-scripts"
    system "npm", "run", "build", "-w", "@outliner/web"

    # Only what the server needs at run time: Node runs the TypeScript sources directly
    # (type stripping), @outliner/core is a local package and ws is the one npm dependency.
    (libexec/"apps/server").install "apps/server/package.json", "apps/server/src"
    (libexec/"packages/core").install "packages/core/package.json", "packages/core/src"
    (libexec/"apps/web").install "apps/web/dist"
    (libexec/"node_modules").install "node_modules/ws"
    (libexec/"node_modules/@outliner").install_symlink libexec/"packages/core"

    (libexec/"bin").install "deploy/homebrew/mynalist"
    (bin/"mynalist").write_env_script libexec/"bin/mynalist",
                                      MYNALIST_NODE: Formula["node"].opt_bin/"node",
                                      MYNALIST_APP:  libexec
  end

  def caveats
    <<~EOS
      Create the config (token, password, port 8080 on 127.0.0.1) once, then start the service:
        mynalist init
        brew services start mynalist

      Open http://127.0.0.1:8080 and log in with the password (`mynalist password`).
      Config: ~/.config/outliner/env   Data and nightly backups: ~/.local/share/outliner/
      These are the same paths as deploy/install.sh, so an existing install's data is picked up.

      For HTTPS from other devices, e.g. through Tailscale Funnel:
        tailscale funnel --bg 8080
    EOS
  end

  service do
    run [opt_bin/"mynalist", "serve"]
    keep_alive true
    log_path var/"log/mynalist.log"
    error_log_path var/"log/mynalist.log"
  end

  test do
    ENV["XDG_CONFIG_HOME"] = testpath/"config"
    ENV["XDG_DATA_HOME"] = testpath/"data"
    port = free_port
    ENV["OUTLINER_PORT"] = port.to_s
    assert_match "Password:", shell_output("#{bin}/mynalist init")

    pid = spawn bin/"mynalist", "serve"
    begin
      health = nil
      30.times do
        sleep 1
        health = shell_output("curl -fsS http://127.0.0.1:#{port}/api/health 2>/dev/null", nil)
        break if health.include?("ok")
      end
      assert_match "\"ok\":true", health
      assert_match "<title>", shell_output("curl -fsS http://127.0.0.1:#{port}/")
    ensure
      Process.kill("TERM", pid)
      Process.wait(pid)
    end

    assert_match "backup", shell_output("#{bin}/mynalist backup")
  end
end
