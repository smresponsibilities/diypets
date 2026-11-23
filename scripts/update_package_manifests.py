import argparse
import hashlib
import json
import os
import subprocess
import sys
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def fetch_sha256(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": "diypets-manifest-updater"})
    h = hashlib.sha256()
    with urllib.request.urlopen(req, timeout=90) as resp:
        while True:
            chunk = resp.read(65536)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest().lower()


def write_text(rel_path: str, content: str) -> None:
    full = os.path.join(ROOT, *rel_path.split("/"))
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8", newline="\n") as f:
        f.write(content.replace("\r\n", "\n"))
    print("Wrote:", rel_path)


def main() -> None:
    parser = argparse.ArgumentParser(description="Update Winget, Homebrew, and Scoop manifests for a DIYPets release.")
    parser.add_argument("--version", required=True, help="Release version without leading v, e.g. 0.4.1")
    parser.add_argument("--date", default="2026-11-24", help="Release date YYYY-MM-DD")
    parser.add_argument("--sha-win", default="", help="Optional precomputed Windows x64 zip SHA-256")
    parser.add_argument("--sha-mac", default="", help="Optional precomputed macOS arm64 tar.gz SHA-256")
    parser.add_argument("--sha-lin", default="", help="Optional precomputed Linux x64 tar.gz SHA-256")
    parser.add_argument("--skip-fetch", action="store_true", help="Skip downloading release assets if hashes are supplied")
    args = parser.parse_args()

    ver = args.version.lstrip("v")
    base = f"https://github.com/smresponsibilities/diypets/releases/download/v{ver}"
    win_url = f"{base}/diypets-x86_64-pc-windows-msvc.zip"
    mac_url = f"{base}/diypets-aarch64-apple-darwin.tar.gz"
    lin_url = f"{base}/diypets-x86_64-unknown-linux-gnu.tar.gz"

    sha_win = args.sha_win.strip().lower() or ("" if args.skip_fetch else fetch_sha256(win_url))
    sha_mac = args.sha_mac.strip().lower() or ("" if args.skip_fetch else fetch_sha256(mac_url))
    sha_lin = args.sha_lin.strip().lower() or ("" if args.skip_fetch else fetch_sha256(lin_url))

    if sha_win:
        scoop_obj = {
            "version": ver,
            "description": "Lightweight cross-platform desktop mascot engine in Rust with Multi-Pet Herd Mode, 2D/SVG Studio, and MCP server.",
            "homepage": "https://shivammahajan.com/diypets/",
            "license": "MIT|Apache-2.0",
            "architecture": {"64bit": {"url": win_url, "hash": sha_win}},
            "bin": ["diypets-creator.exe", "diypets-settings.exe", "diypets.exe"],
            "checkver": {"github": "https://github.com/smresponsibilities/diypets"},
            "autoupdate": {
                "architecture": {
                    "64bit": {
                        "url": "https://github.com/smresponsibilities/diypets/releases/download/v$version/diypets-x86_64-pc-windows-msvc.zip"
                    }
                }
            },
        }
        write_text("packaging/scoop/diypets.json", json.dumps(scoop_obj, indent=4) + "\n")

        wg_dir = os.path.join(ROOT, "packaging", "winget")
        if os.path.isdir(wg_dir):
            for fname in sorted(os.listdir(wg_dir)):
                if not fname.endswith((".yaml", ".yml")):
                    continue
                rel = f"packaging/winget/{fname}"
                with open(os.path.join(ROOT, rel), "r", encoding="utf-8") as f:
                    txt = f.read().replace("\r\n", "\n")
                out_lines = []
                for ln in txt.splitlines():
                    stripped = ln.lstrip()
                    indent = ln[: len(ln) - len(stripped)]
                    if stripped.startswith("PackageVersion:"):
                        out_lines.append(f"{indent}PackageVersion: {ver}")
                    elif stripped.startswith("ReleaseDate:"):
                        out_lines.append(f"{indent}ReleaseDate: {args.date}")
                    elif stripped.startswith("InstallerSha256:"):
                        out_lines.append(f"{indent}InstallerSha256: {sha_win.upper()}")
                    elif stripped.startswith("InstallerUrl:"):
                        out_lines.append(f"{indent}InstallerUrl: {win_url}")
                    elif stripped.startswith("ReleaseNotesUrl:"):
                        out_lines.append(f"{indent}ReleaseNotesUrl: https://github.com/smresponsibilities/diypets/releases/tag/v{ver}")
                    else:
                        out_lines.append(ln)
                write_text(rel, "\n".join(out_lines) + "\n")

    if sha_mac and sha_lin:
        brew_rb = (
            "class Diypets < Formula\n"
            '  desc "Cross-platform desktop mascot engine in Rust with multi-pet herds, 2D/SVG studio, and MCP server"\n'
            '  homepage "https://shivammahajan.com/diypets/"\n'
            f'  version "{ver}"\n'
            "  license any_of: [\"MIT\", \"Apache-2.0\"]\n\n"
            "  on_macos do\n"
            "    on_arm do\n"
            f'      url "{mac_url}"\n'
            f'      sha256 "{sha_mac}"\n'
            "    end\n"
            "  end\n\n"
            "  on_linux do\n"
            "    on_intel do\n"
            f'      url "{lin_url}"\n'
            f'      sha256 "{sha_lin}"\n'
            "    end\n"
            "  end\n\n"
            "  def install\n"
            '    ["diypets", "diypets-settings", "diypets-creator"].each do |name|\n'
            '      found = Dir["**/#{name}"].find { |p| File.file?(p) }\n'
            "      bin.install found if found\n"
            "    end\n"
            "  end\n\n"
            "  test do\n"
            '    assert_predicate bin/"diypets", :exist?\n'
            "  end\n"
            "end\n"
        )
        write_text("packaging/homebrew/diypets.rb", brew_rb)

    # Validate Winget manifest if winget CLI is installed
    wg_full = os.path.join(ROOT, "packaging", "winget")
    if os.path.isdir(wg_full):
        chk = subprocess.run(["winget", "validate", "--manifest", wg_full], text=True)
        if chk.returncode != 0:
            sys.exit(chk.returncode)


if __name__ == "__main__":
    main()
