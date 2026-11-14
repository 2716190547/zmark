#!/bin/bash
# Scan tracked and non-ignored files; only explicitly documented legacy uses are allowed.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 - "$ROOT" "${1:-}" <<'PY'
import pathlib
import re
import subprocess
import sys
import tempfile

root = pathlib.Path(sys.argv[1])
mode = sys.argv[2]
if mode not in ("", "--self-test"):
    sys.exit("usage: check_rename.sh [--self-test]")

rules = []
for line in (root / "scripts/rename_allowlist.txt").read_text().splitlines():
    if not line or line.startswith("#"):
        continue
    path, _, pattern = line.partition("\t")
    rules.append((path, re.compile(pattern) if pattern else None))

legacy = re.compile("glidemark", re.IGNORECASE)

def allowed(path, line):
    return any((path == rule or (rule.endswith("/") and path.startswith(rule)))
               and (pattern is None or pattern.search(line)) for rule, pattern in rules)

def check(base, paths):
    failed = False
    for path in sorted(set(paths)):
        if path in ("scripts/check_rename.sh", "scripts/rename_allowlist.txt"):
            continue
        if legacy.search(path):
            print(f"rename leftover filename: {path}")
            failed = True
        file = base / path
        if not file.exists():  # A tracked file deleted in the current worktree.
            continue
        data = file.read_bytes()  # Read/scan errors must fail, never masquerade as a clean tree.
        if b"\0" in data:
            continue
        for number, line in enumerate(data.decode("utf-8", errors="replace").splitlines(), 1):
            if legacy.search(line) and not allowed(path, line):
                print(f"rename leftover: {path}:{number}:{line}")
                failed = True
    return failed

if mode == "--self-test":
    with tempfile.TemporaryDirectory() as tmp:
        base = pathlib.Path(tmp)
        path = "crates/zmark_core/src/config.rs"
        (base / path).parent.mkdir(parents=True)
        (base / path).write_text('pub const LEGACY_APP_DIR: &str = "GlideMark";\n')
        assert not check(base, [path]), "legacy config migration must remain allowed"
        (base / "ok.rs").write_text('let app = "ZMark";\n')
        assert not check(base, ["ok.rs"])
        for spelling in ("GlideMark", "glidemark", "GLIDEMARK"):
            (base / "bad.rs").write_text(f'let app = "{spelling}";\n')
            assert check(base, ["bad.rs"]), "case-insensitive scan must reject legacy identity"
        (base / path).write_text('let title = "GlideMark";\n')
        assert check(base, [path]), "config allowlist must not hide unrelated identity"
        try:
            check(base, ["."])
        except IsADirectoryError:
            pass
        else:
            raise AssertionError("read errors must not pass")
    print("check_rename self-test: ok")
else:
    result = subprocess.run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
                            cwd=root, check=True, stdout=subprocess.PIPE)
    paths = result.stdout.decode("utf-8").split("\0")
    if check(root, [path for path in paths if path]):
        sys.exit(1)
    print("check_rename: ok")
PY
