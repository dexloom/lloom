"""Live end-to-end script-run test: drives the real `lloom` CLI as a subprocess
against a live server. Skipped unless LLOOM_TEST_LIVE_DB=1 and SURREAL_PASS are
set (same convention as the server tests); also skips if no server answers at
LLOOM_SERVER_URL (default http://127.0.0.1:8000).
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import uuid
from pathlib import Path

import pytest

LIVE = os.environ.get("LLOOM_TEST_LIVE_DB", "") == "1" and bool(os.environ.get("SURREAL_PASS"))
SERVER_URL = os.environ.get("LLOOM_SERVER_URL", "http://127.0.0.1:8000")

pytestmark = pytest.mark.skipif(not LIVE, reason="live e2e needs LLOOM_TEST_LIVE_DB=1 and SURREAL_PASS")


def _server_up() -> bool:
    import urllib.request

    try:
        urllib.request.urlopen(f"{SERVER_URL}/v1/health/live", timeout=2)
        return True
    except Exception:
        return False


def _lloom() -> str:
    bin_name = Path(sys.executable).parent / "lloom"
    if bin_name.exists():
        return str(bin_name)
    found = shutil.which("lloom")
    assert found, "lloom console script not found; run inside `uv run pytest`"
    return found


def _run(args: list[str], cfg: Path, cwd: Path, password: str = "e2e-password-123") -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env.update(
        {
            "LLOOM_CONFIG": str(cfg),
            "LLOOM_SERVER_URL": SERVER_URL,
            "LLOOM_PASSWORD": password,
        }
    )
    env.pop("LLOOM_MAIL_DIR", None)
    return subprocess.run([_lloom(), *args], capture_output=True, text=True, env=env, cwd=cwd, check=False)


def test_live_register_update_send_poll_ack(tmp_path):
    if not _server_up():
        pytest.skip(f"no live server at {SERVER_URL}")
    run_id = uuid.uuid4().hex[:8]
    handle_a, handle_b = f"e2ea_{run_id}", f"e2eb_{run_id}"
    home_a, home_b = tmp_path / "a", tmp_path / "b"
    home_a.mkdir()
    home_b.mkdir()
    cfg_a, cfg_b = home_a / "config.json", home_b / "config.json"

    # A registers headless, updates profile with a real embedding
    r = _run(["register", handle_a, "--description", "e2e sender agent", "--tags", "e2e"], cfg_a, home_a)
    assert r.returncode == 0, r.stderr
    r = _run(["update", "--description", f"e2e sender agent {run_id}", "--tags", "e2e", "--embed"], cfg_a, home_a)
    assert r.returncode == 0, r.stderr
    assert "agent_id" in r.stdout  # PATCH accepted (embedding computed + sent)

    # B registers
    r = _run(["register", handle_b, "--description", "e2e receiver agent"], cfg_b, home_b)
    assert r.returncode == 0, r.stderr

    # A -> B private send lands in sent/ (outbox -> sent on server accept)
    r = _run(["send", "--to", f"@{handle_b}", "live e2e hello"], cfg_a, home_a)
    assert r.returncode == 0, r.stderr
    assert len(list((home_a / ".lloom" / "mail" / "sent").glob("*.md"))) == 1

    # B polls (headless), delivery lands in new/, ack moves it to read/
    r = _run(["poll", "--json"], cfg_b, home_b)
    assert r.returncode == 0, r.stderr
    payload = json.loads(r.stdout.strip().splitlines()[-1])
    deliveries = payload.get("deliveries", [])
    assert any(d["body"] == "live e2e hello" for d in deliveries)
    delivery_id = next(d["delivery_id"] for d in deliveries if d["body"] == "live e2e hello")
    assert len(list((home_b / ".lloom" / "mail" / "new").glob("*.md"))) >= 1

    r = _run(["mail", "ls", "new"], cfg_b, home_b)
    assert "live e2e hello" in r.stdout
    r = _run(["ack", delivery_id], cfg_b, home_b)
    assert r.returncode == 0, r.stderr
    read_files = list((home_b / ".lloom" / "mail" / "read").glob("*.md"))
    assert any("acked_at:" in f.read_text() and "acked_at: null" not in f.read_text() for f in read_files)

    # grep over the mail tree finds the known body
    r = subprocess.run(
        ["grep", "-r", "live e2e hello", str(home_b / ".lloom" / "mail")], capture_output=True, text=True, check=False
    )
    assert r.returncode == 0

    # mail isolation: A's CWD never saw B's inbound mail (A only has its sent copy)
    for folder in ("new", "read"):
        r = _run(["mail", "ls", folder], cfg_a, home_a)
        assert r.stdout.strip() == "no mail", (folder, r.stdout)
