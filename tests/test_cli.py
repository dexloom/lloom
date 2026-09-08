"""CLI tests driving main(argv) with a stubbed Client and tmp CWD/config.

No live server, no network: `lloom.cli.Client` is monkeypatched with a fake
that captures sends; retries run with `time.sleep` patched out.
"""

from __future__ import annotations

import io
import json
import sqlite3
import subprocess
import sys
import time
from typing import ClassVar

import pytest

from lloom import cli
from lloom.client import LloomError
from lloom.config import Config

SECRET = {"api_key": "llm_key", "agent_id": "agent:1", "handle": "@me"}
_REAL_MAYBE_EMBED = cli._maybe_embed  # pristine reference (the fx fixture patches it)


class FakeClient:
    """Captures calls; raises queued exceptions before returning results."""

    def __init__(self, server_url, api_key=None, timeout=30.0):
        self.server_url = server_url
        self.api_key = api_key
        self.state["clients"] += 1
        self.state.setdefault("urls", []).append(server_url)

    state: ClassVar[dict] = {}

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return None

    def close(self):
        return None

    def _maybe_raise(self):
        errors = self.state.get("send_errors")
        if errors:
            raise errors.pop(0)

    # auth
    def register(self, handle, password, description=None, tags=None, location=None, needs=None, offers=None, challenge=None):
        call = {"handle": handle, "password": password, "description": description, "tags": tags, "location": location, "needs": needs, "offers": offers, "challenge": challenge}
        self.state["register"] = call
        self.state.setdefault("registers", []).append(call)
        # `register_error` fails every attempt; `register_errors` is a queue,
        # for the retry paths that need attempt 1 and attempt 2 to differ
        queued = self.state.get("register_errors")
        if queued:
            raise queued.pop(0)
        err = self.state.get("register_error")
        if err:
            raise err
        return {"agent_id": "agent:2", "handle": handle, "api_key": "llm_new"}

    def check_handle(self, handle):
        self.state["check_handle"] = handle
        return dict(self.state["check_handle_result"])

    def login(self, handle, password):
        self.state["login"] = {"handle": handle, "password": password}
        return {"agent_id": "agent:1", "handle": handle, "api_key": "llm_rot"}

    def rotate(self, handle, password):
        self.state["rotate"] = {"handle": handle, "password": password}
        return {"api_key": "llm_rot2"}

    def whoami(self):
        # `whoami_result` lets a test drive the tier/score/quota/moderation
        # summary; the default stays the minimal shape an older hub returns,
        # so every existing test still exercises the "server predates tiers"
        # path where the summary line is skipped entirely.
        return dict(self.state.get("whoami_result") or {"agent_id": "agent:1", "handle": "@me"})

    def reputation(self):
        self.state["reputation_calls"] = self.state.get("reputation_calls", 0) + 1
        return dict(self.state.get("reputation_result") or {})

    def update_agent(self, agent_id, **fields):
        self.state.setdefault("updates", []).append({"agent_id": agent_id, **fields})
        return {"agent_id": agent_id, **fields}

    # messaging
    def send(self, payload):
        self.state.setdefault("sends", []).append(payload)
        self._maybe_raise()
        # the server's replay protection, modelled: a key it has already
        # committed echoes the first result with `replayed: true` and
        # delivers nothing new
        key = payload.get("idempotency_key")
        committed = self.state.setdefault("committed", {})
        if key and key in committed:
            return {**committed[key], "replayed": True}
        res = dict(self.state.get("send_result") or {"message_id": "message:1", "recipient_count": 1})
        if key:
            committed[key] = dict(res)
        return res

    def mailbox(self, limit=50, cursor=None, wait=0):
        self.state.setdefault("mailbox_calls", []).append({"limit": limit, "cursor": cursor, "wait": wait})
        return dict(self.state["mailbox_result"])

    def ack(self, delivery_id):
        self.state.setdefault("acks", []).append(delivery_id)
        return {"delivery_id": delivery_id, "state": "acked"}

    def public(self, limit=50):
        return self.state.get("public_result") or {"messages": []}

    def feedback(self, verdict, *, delivery_id=None, message_id=None, note=None, block=None):
        call = {
            "verdict": verdict,
            "delivery_id": delivery_id,
            "message_id": message_id,
            "note": note,
            "block": block,
        }
        self.state.setdefault("feedback", []).append(call)
        errors = self.state.get("feedback_errors")
        if errors:
            raise errors.pop(0)
        return {
            "feedback_id": "feedback:1",
            "verdict": verdict,
            "weight_applied": self.state.get("feedback_weight", 0.5),
            "state": "open" if verdict not in ("helpful", "not_helpful") else "recorded",
            "blocked": bool(verdict not in ("helpful", "not_helpful") and block is not False),
            "auto_restricted": False,
        }

    def block(self, handle):
        self.state.setdefault("blocks", []).append(handle)
        return {"agent_id": "agent:2", "handle": handle.lstrip("@"), "blocked": True, "changed": True}

    def unblock(self, handle):
        self.state.setdefault("unblocks", []).append(handle)
        return {"agent_id": "agent:2", "handle": handle.lstrip("@"), "blocked": False, "changed": True}

    def blocks(self, limit=50):
        self.state["blocks_listed"] = {"limit": limit}
        return {"blocks": [], "next_cursor": None}

    def routing(self, message_id):
        self.state.setdefault("routing_calls", []).append(message_id)
        err = self.state.get("routing_error")
        if err:
            raise err
        return dict(self.state["routing_result"])


@pytest.fixture
def fx(monkeypatch, tmp_path):
    """Isolated CLI harness: tmp CWD (mail root), tmp config, fake client."""
    FakeClient.state = {"clients": 0}
    monkeypatch.setattr(cli, "Client", FakeClient)
    monkeypatch.setattr(cli, "_maybe_embed", lambda body: [0.1, 0.2])
    monkeypatch.setattr(cli.time, "sleep", lambda s: FakeClient.state.setdefault("sleeps", []).append(s))
    monkeypatch.setattr(cli, "legacy_home_store", lambda: tmp_path / "home-store.db")
    monkeypatch.setattr(sys, "stdin", io.StringIO(""))
    monkeypatch.delenv("LLOOM_PASSWORD", raising=False)
    monkeypatch.delenv("LLOOM_SERVER_URL", raising=False)
    monkeypatch.delenv("LLOOM_MAIL_DIR", raising=False)
    monkeypatch.delenv("LLOOM_CONFIG", raising=False)
    monkeypatch.chdir(tmp_path)
    cfg = tmp_path / "config.json"
    cfg.write_text(json.dumps(SECRET))
    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": None}
    return cfg


def _mail_root():
    from pathlib import Path

    return Path.cwd() / ".lloom" / "mail"


def _files(folder):
    return sorted(((_mail_root() / folder)).glob("*.md"))


def _within_jitter(sleeps, unjittered):
    """The backoff is scaled by uniform(0.5, 1.5), so a recorded sleep is only
    pinned to its band around the deterministic delay it came from."""
    lo, hi = 1.0 - cli.RETRY_JITTER, 1.0 + cli.RETRY_JITTER
    return len(sleeps) == len(unjittered) and all(base * lo <= s <= base * hi for s, base in zip(sleeps, unjittered))


# -- P6: headless auth -------------------------------------------------------


def test_register_update_broadcast_headless_with_env_password(fx, capsys):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@newbie", "--description", "ops agent", "--tags", "ops,ci"])
    assert FakeClient.state["register"]["password"] == "supersecret"
    saved = json.loads(fx.read_text())
    assert saved["api_key"] == "llm_new"
    assert saved["agent_id"] == "agent:2"

    cli.main(["--config", str(fx), "update", "--description", "senior ops agent", "--tags", "ops", "--embed"])
    upd = FakeClient.state["updates"][-1]
    assert upd["description"] == "senior ops agent"
    assert upd["tags"] == ["ops"]
    assert upd["embedding"] == [0.1, 0.2]

    cli.main(["--config", str(fx), "broadcast", "hello world"])
    payload = FakeClient.state["sends"][-1]
    assert payload["kind"] == "broadcast"
    assert payload["body"] == "hello world"
    assert len(_files("sent")) == 1


def test_password_stdin_flag(fx):
    sys.stdin = io.StringIO("pw-from-stdin\n")
    cli.main(["--config", str(fx), "login", "@me", "--password-stdin"])
    assert FakeClient.state["login"]["password"] == "pw-from-stdin"


def test_no_password_source_headless_exits_2(fx):
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "login", "@me"])
    assert ei.value.code == 2


def test_register_and_login_merge_config_not_replace(fx, monkeypatch):
    """F3 (review): register/login used to REPLACE the whole config file,
    silently dropping user keys (mail_dir, server_url customization).
    They must merge over the loaded config."""
    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    cli.main(["--config", str(fx), "config", "set", "mail-dir", "./custom/mail"])
    cli.main(["--config", str(fx), "config", "set", "server_url", "http://example:9"])

    cli.main(["--config", str(fx), "register", "@newbie"])
    saved = json.loads(fx.read_text())
    assert saved["api_key"] == "llm_new"
    assert saved["agent_id"] == "agent:2"
    assert saved["handle"] == "@newbie"
    assert saved["mail_dir"] == "./custom/mail"
    assert saved["server_url"] == "http://example:9"

    cli.main(["--config", str(fx), "login", "@me"])
    saved = json.loads(fx.read_text())
    assert saved["api_key"] == "llm_rot"
    assert saved["handle"] == "@me"
    assert saved["mail_dir"] == "./custom/mail"
    assert saved["server_url"] == "http://example:9"


# -- P6: retry policy ----------------------------------------------------------


def test_send_4xx_parks_dead_no_retries(fx, capsys):
    FakeClient.state["send_errors"] = [LloomError("bad_vector_profile", "bad vector", 422)]
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "send", "--to", "@other", "hi"])
    assert ei.value.code == 1
    assert FakeClient.state.get("sleeps", []) == []  # no rethrow loop
    assert FakeClient.state["clients"] == 1
    assert len(_files("outbox")) == 1
    text = _files("outbox")[0].read_text()
    assert "dead: true" in text
    assert "bad_vector_profile" in text


def test_send_429_retries_with_backoff_reusing_one_client(fx):
    FakeClient.state["send_errors"] = [LloomError("rate_limited", "slow down", 429), LloomError("rate_limited", "slow down", 429)]
    cli.main(["--config", str(fx), "send", "--to", "@other", "hi"])
    assert _within_jitter(FakeClient.state["sleeps"], [1.0, 2.0])
    assert FakeClient.state["clients"] == 1  # ONE client reused across attempts
    keys = {p["idempotency_key"] for p in FakeClient.state["sends"]}
    assert len(keys) == 1  # same idempotency key across attempts
    assert len(_files("sent")) == 1


def test_send_5xx_and_network_error_backoff_then_success(fx):
    FakeClient.state["send_errors"] = [RuntimeError("connection refused"), LloomError("internal", "boom", 500)]
    cli.main(["--config", str(fx), "send", "--to", "@other", "hi"])
    assert _within_jitter(FakeClient.state["sleeps"], [1.0, 2.0])
    assert len(_files("sent")) == 1


def test_retry_after_header_honored(fx):
    FakeClient.state["send_errors"] = [LloomError("rate_limited", "slow down", 429, retry_after=0.25)]
    cli.main(["--config", str(fx), "send", "--to", "@other", "hi"])
    assert FakeClient.state["sleeps"] == [0.25]


def test_retry_exhaustion_keeps_entry_in_outbox(fx):
    FakeClient.state["send_errors"] = [RuntimeError("connection refused")] * 5
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "send", "--to", "@other", "hi"])
    assert ei.value.code == 1
    assert len(FakeClient.state["sleeps"]) == 4
    assert len(_files("outbox")) == 1
    assert "dead: false" in _files("outbox")[0].read_text()  # still retryable


# -- P6/P7: retry command -------------------------------------------------------


def test_server_down_then_retry_moves_to_sent_with_same_key(fx, capsys):
    from lloom.mailstore import MailStore

    FakeClient.state["send_errors"] = [RuntimeError("connection refused")] * 5
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "send", "--to", "@other", "durable hello"])
    assert len(_files("outbox")) == 1
    original_key = MailStore(_mail_root()).list("outbox")[0]["idempotency_key"]

    FakeClient.state["send_errors"] = []
    cli.main(["--config", str(fx), "retry"])
    assert len(_files("sent")) == 1
    assert _files("outbox") == []
    resent = FakeClient.state["sends"][-1]
    assert resent["idempotency_key"] == original_key  # idempotency key reused
    assert resent["body"] == "durable hello"


def test_retry_reports_and_drops_poisoned_entries(fx, capsys):
    FakeClient.state["send_errors"] = [LloomError("recipient_not_found", "no such handle", 404)]
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "send", "--to", "@ghost", "poison"])
    FakeClient.state["send_errors"] = []
    n_sends = len(FakeClient.state.get("sends", []))

    cli.main(["--config", str(fx), "retry"])
    out = capsys.readouterr().out
    assert "dropped" in out
    assert len(FakeClient.state.get("sends", [])) == n_sends  # never resent
    assert _files("outbox") == []  # dropped, not parked forever


# -- A6: retry hygiene (jitter, --max budget, consecutive-429 stop) -------------


def _park(n: int) -> list[str]:
    """Park `n` retryable outbox entries, oldest first. `created_at` is set
    explicitly because `MailStore.list` orders by it, and these tests assert
    exactly WHICH entries a bounded run attempts."""
    from lloom.mailstore import MailStore

    store = MailStore(_mail_root())
    return [
        store.enqueue({
            "from": "@me",
            "to": "@other",
            "kind": "private",
            "body": f"parked {i:03d}",
            "idempotency_key": f"key-{i:03d}",
            "created_at": 1000.0 + i,
        })
        for i in range(n)
    ]


def test_backoff_is_jittered_but_retry_after_is_verbatim():
    """One restart knocks a whole fleet offline at the same instant; an
    unjittered backoff then brings it all back on the same second."""
    samples = [cli._backoff_delay(2, None) for _ in range(200)]
    assert all(1.0 <= s <= 3.0 for s in samples)  # 2 s scaled by uniform(0.5, 1.5)
    assert not all(s == 2.0 for s in samples)  # the lockstep delay is gone
    assert len(set(samples)) > 1

    # Retry-After is an instruction from the server, not an estimate: verbatim
    ra = LloomError("rate_limited", "slow down", 429, retry_after=7.0)
    assert [cli._backoff_delay(a, ra) for a in (1, 2, 3)] == [7.0, 7.0, 7.0]


def test_retry_max_bounds_the_run_and_names_what_is_left(fx, capsys):
    _park(60)
    cli.main(["--config", str(fx), "retry", "--max", "50"])
    out = capsys.readouterr().out
    assert len(FakeClient.state["sends"]) == 50  # a bounded run, not a storm
    assert len(_files("sent")) == 50
    assert len(_files("outbox")) == 10
    assert "10 entries left in the outbox" in out
    # the OLDEST 50 go first; the tail waits for the next invocation
    assert {p["idempotency_key"] for p in FakeClient.state["sends"]} == {f"key-{i:03d}" for i in range(50)}


def test_retry_default_max_is_50(fx):
    _park(60)
    cli.main(["--config", str(fx), "retry"])
    assert len(FakeClient.state["sends"]) == cli.RETRY_RUN_MAX
    assert len(_files("outbox")) == 10


def test_retry_stops_after_three_consecutive_429s(fx, capsys):
    from lloom.mailstore import MailStore

    _park(4)
    # every entry burns RETRY_MAX_ATTEMPTS 429s inside _deliver before it gives up
    FakeClient.state["send_errors"] = [LloomError("rate_limited", "slow down", 429)] * (cli.RETRY_MAX_ATTEMPTS * 3)
    cli.main(["--config", str(fx), "retry"])
    cap = capsys.readouterr()

    assert len(FakeClient.state["sends"]) == cli.RETRY_MAX_ATTEMPTS * 3  # the 4th is never attempted
    assert "stopped after 3 rate limits (429) in a row" in cap.err
    assert "4 entries left in the outbox" in cap.out
    parked = MailStore(_mail_root()).list("outbox")
    assert len(parked) == 4  # nothing sent, nothing dropped
    assert [e.get("error") is not None for e in parked] == [True, True, True, False]


def test_retry_429_counter_resets_on_any_other_outcome(fx, capsys):
    """Three 429s SPREAD OUT are not a throttling hub: only a run of them is."""
    _park(4)
    rl = [LloomError("rate_limited", "slow down", 429)] * cli.RETRY_MAX_ATTEMPTS
    boom = [LloomError("internal", "boom", 500)] * cli.RETRY_MAX_ATTEMPTS
    FakeClient.state["send_errors"] = [*rl, *rl, *boom, *rl]  # 429, 429, 500, 429
    cli.main(["--config", str(fx), "retry"])
    cap = capsys.readouterr()

    assert "stopped after" not in cap.err  # three 429s, but never three in a row
    assert len(FakeClient.state["sends"]) == cli.RETRY_MAX_ATTEMPTS * 4  # all four tried
    assert len(_files("outbox")) == 4
    assert "4 entries left in the outbox" in cap.out


def test_retry_rejects_a_max_below_one(fx):
    _park(1)
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "retry", "--max", "0"])
    assert ei.value.code == 2
    assert len(_files("outbox")) == 1


# -- P6: poll cursor + json ------------------------------------------------------


def test_poll_json_cursor_persisted_and_reused(fx, capsys):
    FakeClient.state["mailbox_result"] = {
        "deliveries": [{"delivery_id": "d:1", "message_id": "m:1", "sender": "@other", "kind": "private", "body": "hi", "state": "read", "created_at": "2026-08-24T00:00:01Z"}],
        "next_cursor": "CURSOR_1",
    }
    cli.main(["--config", str(fx), "poll", "--json"])
    parsed = json.loads(capsys.readouterr().out.strip().splitlines()[-1])
    assert parsed["deliveries"][0]["delivery_id"] == "d:1"
    cursor_file = fx.parent / "state" / fx.name / "cursor.txt"
    assert cursor_file.read_text() == "CURSOR_1"

    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": None}
    cli.main(["--config", str(fx), "poll"])
    assert FakeClient.state["mailbox_calls"][-1]["cursor"] == "CURSOR_1"  # stored cursor reused

    cli.main(["--config", str(fx), "poll", "--cursor", "OVERRIDE"])
    assert FakeClient.state["mailbox_calls"][-1]["cursor"] == "OVERRIDE"  # --cursor overrides


def _mail(delivery_id, created_at, *, message_id="m:1", body="hi"):
    return {
        "delivery_id": delivery_id,
        "message_id": message_id,
        "sender": "@other",
        "kind": "private",
        "body": body,
        "state": "read",
        "created_at": created_at,
    }


def test_a_synthesized_cursor_never_moves_backwards(fx):
    """A lease repair arrives BEHIND the stored cursor; following it would
    rewind and re-serve every un-acked delivery in between."""
    cursor_file = fx.parent / "state" / fx.name / "cursor.txt"
    FakeClient.state["mailbox_result"] = {
        "deliveries": [_mail("delivery:9", "2026-08-24T00:00:09Z", message_id="m:9")],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    ahead = cursor_file.read_text()
    assert ahead == cli._encode_client_cursor("2026-08-24T00:00:09Z", "delivery:9")

    # a reverted delivery from earlier in the queue, on a page with nothing
    # newer on it and no next_cursor to follow
    FakeClient.state["mailbox_result"] = {
        "deliveries": [_mail("delivery:2", "2026-08-24T00:00:02Z", message_id="m:2")],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    assert cursor_file.read_text() == ahead, "repair rewound the cursor"

    # a genuinely newer delivery still advances it
    FakeClient.state["mailbox_result"] = {
        "deliveries": [_mail("delivery:11", "2026-08-24T00:00:11Z", message_id="m:11")],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    assert cursor_file.read_text() == cli._encode_client_cursor(
        "2026-08-24T00:00:11Z", "delivery:11"
    )


def test_a_server_that_echoes_its_cursor_is_written_verbatim(fx):
    """The server's own repair-only answer: `next_cursor` equal to what was
    sent. It takes the `if next_cursor` branch, so it is stored as given."""
    cursor_file = fx.parent / "state" / fx.name / "cursor.txt"
    cursor_file.parent.mkdir(parents=True, exist_ok=True)
    stored = cli._encode_client_cursor("2026-08-24T00:00:09Z", "delivery:9")
    cursor_file.write_text(stored)
    FakeClient.state["mailbox_result"] = {
        "deliveries": [_mail("delivery:2", "2026-08-24T00:00:02Z", message_id="m:2")],
        "next_cursor": stored,
    }
    cli.main(["--config", str(fx), "poll"])
    assert cursor_file.read_text() == stored


def test_cursor_advances_only_forward():
    older = cli._encode_client_cursor("2026-08-24T00:00:02Z", "delivery:2")
    newer = cli._encode_client_cursor("2026-08-24T00:00:09Z", "delivery:9")
    same_time_lower = cli._encode_client_cursor("2026-08-24T00:00:09Z", "delivery:1")
    assert cli._cursor_advances(older, newer)
    assert not cli._cursor_advances(newer, older)
    assert not cli._cursor_advances(newer, newer), "equal is not forward"
    assert not cli._cursor_advances(newer, same_time_lower), "id breaks the tie"
    assert cli._cursor_advances(None, older), "nothing stored yet"
    assert cli._cursor_advances("not-a-cursor", older), "unreadable is not worth keeping"


def test_two_configs_never_share_cursor_state(fx, tmp_path):
    persona_dir = tmp_path / "persona-b"
    persona_dir.mkdir()
    cfg_b = persona_dir / "config.json"
    cfg_b.write_text(json.dumps({**SECRET, "handle": "@b"}))
    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": "CUR_A"}
    cli.main(["--config", str(fx), "poll"])
    cli.main(["--config", str(cfg_b), "poll"])
    # B had no stored cursor of its own -> polled from the epoch sentinel,
    # never with A's cursor
    assert FakeClient.state["mailbox_calls"][1]["cursor"] == cli.EPOCH_CURSOR
    # each config keeps its own cursor under its own state dir
    assert (fx.parent / "state" / fx.name / "cursor.txt").read_text() == "CUR_A"
    assert (persona_dir / "state" / cfg_b.name / "cursor.txt").read_text() == "CUR_A"
    assert Config(fx).state_dir() != Config(cfg_b).state_dir()


def test_two_configs_in_same_directory_never_share_state(fx, tmp_path):
    """Round-16 review: `alice.json` + `bob.json` in ONE directory must not
    share cursor.txt — B reusing A's cursor would permanently skip B's
    older pending deliveries."""
    cfg_b = fx.parent / "bob.json"
    cfg_b.write_text(json.dumps({**SECRET, "handle": "@bob"}))
    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": "CUR_A"}
    cli.main(["--config", str(fx), "poll"])
    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": "CUR_B"}
    cli.main(["--config", str(cfg_b), "poll"])
    # B polled from the epoch sentinel, not A's stored cursor
    assert FakeClient.state["mailbox_calls"][1]["cursor"] == cli.EPOCH_CURSOR
    state_a = fx.parent / "state" / fx.name
    state_b = fx.parent / "state" / cfg_b.name
    assert (state_a / "cursor.txt").read_text() == "CUR_A"
    assert (state_b / "cursor.txt").read_text() == "CUR_B"


def test_epoch_cursor_encodes_pre_epoch_sentinel():
    """The first-poll sentinel must decode to (created_at, id) strictly
    before every possible delivery — the server's cursor keyset format."""
    import base64

    raw = base64.urlsafe_b64decode(cli.EPOCH_CURSOR.encode()).decode()
    assert raw == "1970-01-01T00:00:00Z|delivery:0"


def test_first_poll_without_cursor_recovers_read_but_unfiled_backlog(fx, capsys):
    """F2 (review): a crash between the server marking items read and local
    filing/cursor persistence leaves deliveries read-but-unfiled. The
    no-cursor server path only claims PENDING items (backlog invisible
    until lease revert), so a first poll MUST send the epoch sentinel —
    the cursor-page path returns ALL un-acked deliveries, and filing is
    idempotent (content-addressed files)."""
    FakeClient.state["mailbox_result"] = {
        "deliveries": [
            {
                "delivery_id": "d:1",
                "message_id": "m:1",
                "sender": "@other",
                "kind": "private",
                "body": "backlog mail",
                "state": "read",
            }
        ],
        "next_cursor": "CUR_AFTER_BACKLOG",
    }
    # no cursor file exists: poll must carry a non-empty cursor (epoch)
    cli.main(["--config", str(fx), "poll"])
    first = FakeClient.state["mailbox_calls"][0]
    assert first["cursor"]  # non-empty cursor param, never the no-cursor path
    assert first["cursor"] == cli.EPOCH_CURSOR
    # the read-but-unfiled delivery was filed into new/
    assert len(_files("new")) == 1
    # cursor persisted only after filing
    assert (fx.parent / "state" / fx.name / "cursor.txt").read_text() == "CUR_AFTER_BACKLOG"

    # second poll (cursor now stored) pages from the stored cursor, does
    # not re-file anything
    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": None}
    cli.main(["--config", str(fx), "poll"])
    assert FakeClient.state["mailbox_calls"][-1]["cursor"] == "CUR_AFTER_BACKLOG"
    assert len(_files("new")) == 1


def test_partial_first_page_synthesizes_cursor_at_last_delivery(fx):
    """F2 companion: the server emits next_cursor only on a FULL page. A
    partial first (epoch) page must still persist a cursor — synthesized
    from the last returned delivery's (created_at, id), the same keyset the
    server encodes — or every later poll would re-walk from the epoch."""
    import base64

    FakeClient.state["mailbox_result"] = {
        "deliveries": [
            {
                "delivery_id": "delivery:1",
                "message_id": "m:1",
                "sender": "@other",
                "kind": "private",
                "body": "only mail",
                "state": "read",
                "created_at": "2026-08-24T00:00:05Z",
            }
        ],
        "next_cursor": None,  # partial page: server sends no cursor
    }
    cli.main(["--config", str(fx), "poll"])
    cursor_file = fx.parent / "state" / fx.name / "cursor.txt"
    assert cursor_file.exists()
    raw = base64.urlsafe_b64decode(cursor_file.read_text().encode()).decode()
    assert raw == "2026-08-24T00:00:05Z|delivery:1"
    # the next poll resumes from the synthesized cursor, not the epoch
    cli.main(["--config", str(fx), "poll"])
    assert FakeClient.state["mailbox_calls"][-1]["cursor"] == cursor_file.read_text()


def test_empty_first_poll_stores_no_cursor(fx):
    """Nothing un-acked and no stored cursor: no cursor file is written (a
    later poll re-pages from the epoch, which is free on an empty mailbox)."""
    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": None}
    cli.main(["--config", str(fx), "poll"])
    assert FakeClient.state["mailbox_calls"][0]["cursor"] == cli.EPOCH_CURSOR
    assert not (fx.parent / "state" / fx.name / "cursor.txt").exists()


def test_poll_cursor_not_advanced_when_filing_fails(fx, monkeypatch):
    """Cursor is persisted only AFTER all deliveries are filed into the
    maildir: a filing failure must leave the cursor un-advanced so the next
    poll re-serves the tail (at-least-once)."""
    from lloom.mailstore import MailStore

    FakeClient.state["mailbox_result"] = {
        "deliveries": [{"delivery_id": "d:9", "message_id": "m:9", "sender": "@other", "kind": "private", "body": "unfiled", "state": "read"}],
        "next_cursor": "CURSOR_X",
    }

    def _boom(self, mail):
        raise RuntimeError("disk full")

    monkeypatch.setattr(MailStore, "receive", _boom)
    with pytest.raises(RuntimeError):
        cli.main(["--config", str(fx), "poll", "--json"])
    assert not (fx.parent / "state" / fx.name / "cursor.txt").exists()


def test_partial_page_after_stored_cursor_advances_cursor(fx):
    """F1 (review): the server emits next_cursor only on FULL pages. A
    partial page on a LATER poll (stored cursor, not the epoch sentinel)
    returns next_cursor=None — without synthesizing the continuation
    cursor at the last delivery, the prior cursor would persist and every
    subsequent poll re-serve the same un-acked tail forever."""
    import base64

    # poll 1 (epoch sentinel): FULL page — the server supplies its own cursor
    FakeClient.state["mailbox_result"] = {
        "deliveries": [
            {
                "delivery_id": "delivery:1",
                "message_id": "m:1",
                "sender": "@other",
                "kind": "private",
                "body": "full page mail 1",
                "state": "read",
                "created_at": "2026-08-24T00:00:01Z",
            },
            {
                "delivery_id": "delivery:2",
                "message_id": "m:2",
                "sender": "@other",
                "kind": "private",
                "body": "full page mail 2",
                "state": "read",
                "created_at": "2026-08-24T00:00:02Z",
            },
        ],
        "next_cursor": "CURSOR_FULL_PAGE",
    }
    cli.main(["--config", str(fx), "poll"])
    cursor_file = fx.parent / "state" / fx.name / "cursor.txt"
    assert cursor_file.read_text() == "CURSOR_FULL_PAGE"

    # poll 2 (stored cursor, NOT the epoch): PARTIAL page — next_cursor=None
    FakeClient.state["mailbox_result"] = {
        "deliveries": [
            {
                "delivery_id": "delivery:3",
                "message_id": "m:3",
                "sender": "@other",
                "kind": "private",
                "body": "partial tail mail",
                "state": "read",
                "created_at": "2026-08-24T00:00:03Z",
            }
        ],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    assert FakeClient.state["mailbox_calls"][-1]["cursor"] == "CURSOR_FULL_PAGE"
    # the cursor file ADVANCED: synthesized at the partial page's last delivery
    synthesized = cursor_file.read_text()
    raw = base64.urlsafe_b64decode(synthesized.encode()).decode()
    assert raw == "2026-08-24T00:00:03Z|delivery:3"

    # poll 3: the cursor param resumes AFTER the partial page's last delivery
    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": None}
    cli.main(["--config", str(fx), "poll"])
    assert FakeClient.state["mailbox_calls"][-1]["cursor"] == synthesized

    # empty poll: no-op — the cursor file is left unchanged
    assert cursor_file.read_text() == synthesized


def test_poll_files_delivery_with_empty_body(fx):
    """F2 (review): the server schema rejects empty bodies at POST time,
    but deliveries already in flight (older servers) may carry body="" —
    poll must still file them; a required-non-empty body check would wedge
    the poll loop on an unfileable delivery."""
    from lloom.mailstore import MailStore

    FakeClient.state["mailbox_result"] = {
        "deliveries": [
            {
                "delivery_id": "d:empty",
                "message_id": "m:1",
                "sender": "@other",
                "kind": "private",
                "body": "",
                "state": "read",
                "created_at": "2026-08-24T00:00:01Z",
            }
        ],
        "next_cursor": "CUR",
    }
    cli.main(["--config", str(fx), "poll"])
    assert len(_files("new")) == 1
    mail = MailStore(_mail_root()).list("new")[0]
    assert mail["body"] == ""
    assert mail["delivery_id"] == "d:empty"
    # the cursor advanced past the filed delivery
    assert (fx.parent / "state" / fx.name / "cursor.txt").read_text() == "CUR"


# -- P6: config command + server url ----------------------------------------------


def test_config_set_and_show_redacts_api_key(fx, capsys):
    cli.main(["--config", str(fx), "config", "set", "server-url", "http://elsewhere:9000"])
    assert json.loads(fx.read_text())["server_url"] == "http://elsewhere:9000"
    cli.main(["--config", str(fx), "config", "show"])
    out = capsys.readouterr().out
    assert "llm_key" not in out
    assert "****" in out


def test_server_url_resolution_flag_config_env_default(fx, monkeypatch):
    # default: the public hub, so a fresh install needs no configuration
    cli.main(["--config", str(fx), "whoami"])
    assert _last_client_url() == "https://api.lloom.xyz"
    # env beats default
    monkeypatch.setenv("LLOOM_SERVER_URL", "http://env-server:8000")
    cli.main(["--config", str(fx), "whoami"])
    assert _last_client_url() == "http://env-server:8000"
    # config beats env
    cli.main(["--config", str(fx), "config", "set", "server-url", "http://cfg-server:8000"])
    cli.main(["--config", str(fx), "whoami"])
    assert _last_client_url() == "http://cfg-server:8000"
    # flag beats config
    cli.main(["--config", str(fx), "--server", "http://flag-server:8000", "whoami"])
    assert _last_client_url() == "http://flag-server:8000"


def _last_client_url():
    urls = FakeClient.state.get("urls", [])
    return urls[-1]


# -- P7: maildir flows ---------------------------------------------------------------


def test_poll_files_into_new_then_read_and_ack(fx, capsys):
    FakeClient.state["mailbox_result"] = {
        "deliveries": [{"delivery_id": "d:1", "message_id": "m:1", "sender": "@other", "kind": "private", "body": "first mail", "state": "read", "created_at": "2026-08-24T00:00:01Z"}],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    assert len(_files("new")) == 1
    mail_id = _files("new")[0].stem

    cli.main(["--config", str(fx), "mail", "read", mail_id[:8]])
    out = capsys.readouterr().out
    assert "first mail" in out
    assert len(_files("read")) == 1  # reading IS filing
    assert _files("new") == []

    FakeClient.state["mailbox_result"] = {
        "deliveries": [{"delivery_id": "d:2", "message_id": "m:2", "sender": "@other", "kind": "private", "body": "second mail", "state": "read", "created_at": "2026-08-24T00:00:02Z"}],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    assert len(_files("new")) == 1
    cli.main(["--config", str(fx), "ack", "d:2"])
    assert FakeClient.state["acks"] == ["d:2"]
    assert _files("new") == []
    assert len(_files("read")) == 2
    acked = [f for f in _files("read") if "acked_at" in f.read_text() and "acked_at: null" not in f.read_text()]
    assert len(acked) == 1


def test_two_cwds_never_see_each_others_mail(fx, tmp_path, monkeypatch):
    FakeClient.state["mailbox_result"] = {
        "deliveries": [{"delivery_id": "d:1", "message_id": "m:1", "sender": "@other", "kind": "private", "body": "for A", "state": "read", "created_at": "2026-08-24T00:00:01Z"}],
        "next_cursor": None,
    }
    dir_a = tmp_path / "a"
    dir_b = tmp_path / "b"
    dir_a.mkdir()
    dir_b.mkdir()
    monkeypatch.chdir(dir_a)
    cli.main(["--config", str(fx), "poll"])
    assert len(_files("new")) == 1
    monkeypatch.chdir(dir_b)
    cli.main(["--config", str(fx), "mail", "ls"])
    assert len(_files("new")) == 0
    assert len(_files("read")) == 0
    assert len(_files("sent")) == 0


def test_legacy_sqlite_outbox_migrated_on_first_mail_op(fx, capsys):
    state_dir = fx.parent / "state"
    state_dir.mkdir(exist_ok=True)
    db = state_dir / "store.db"
    conn = sqlite3.connect(db)
    conn.execute(
        "CREATE TABLE outbox (id TEXT PRIMARY KEY, created_at REAL, payload TEXT NOT NULL,"
        " status TEXT NOT NULL, attempts INTEGER DEFAULT 0, message_id TEXT, error TEXT)"
    )
    conn.execute(
        "INSERT INTO outbox (id, created_at, payload, status) VALUES (?, ?, ?, 'queued')",
        ("legacy-1", 1.0, json.dumps({"kind": "private", "to": "@other", "body": "legacy hello", "idempotency_key": "legacy-key"})),
    )
    conn.execute(
        "INSERT INTO outbox (id, created_at, payload, status) VALUES (?, ?, ?, 'dead')",
        ("legacy-2", 2.0, json.dumps({"kind": "private", "to": "@other", "body": "dead one", "idempotency_key": "dead-key"})),
    )
    conn.commit()
    conn.close()

    cli.main(["--config", str(fx), "mail", "ls"])  # first CLI mail op drains it
    out = capsys.readouterr().out
    assert "legacy hello" in out
    assert "dead one" not in out  # dead rows are not drained
    assert len(_files("outbox")) == 1
    assert (state_dir / "store.db.migrated").exists()
    assert not (state_dir / "store.db").exists()


def test_legacy_sqlite_outbox_with_embedding_migrates_sidecar(fx, capsys):
    """Review F2 (round 8): a legacy embedded send that committed without
    confirmation must retry with the EXACT stored vector — the migration
    writes a vec sidecar instead of discarding it."""
    state_dir = fx.parent / "state"
    state_dir.mkdir(exist_ok=True)
    db = state_dir / "store.db"
    conn = sqlite3.connect(db)
    conn.execute(
        "CREATE TABLE outbox (id TEXT PRIMARY KEY, created_at REAL, payload TEXT NOT NULL,"
        " status TEXT NOT NULL, attempts INTEGER DEFAULT 0, message_id TEXT, error TEXT)"
    )
    conn.execute(
        "INSERT INTO outbox (id, created_at, payload, status) VALUES (?, ?, ?, 'queued')",
        (
            "legacy-emb",
            1.0,
            json.dumps(
                {
                    "kind": "private",
                    "to": "@other",
                    "body": "legacy embedded",
                    "idempotency_key": "legacy-emb-key",
                    "embedding": [0.25, 0.75],
                }
            ),
        ),
    )
    conn.commit()
    conn.close()

    cli.main(["--config", str(fx), "mail", "ls"])
    out = capsys.readouterr().out
    assert "legacy embedded" in out
    assert len(_files("outbox")) == 1
    mail_id = _files("outbox")[0].stem
    sidecar = _mail_root() / "outbox" / f"{mail_id}.vec.json"
    assert json.loads(sidecar.read_text()) == {"embedding": [0.25, 0.75], "backend": "legacy"}
    assert "embed: true" in _files("outbox")[0].read_text()


def test_mail_search_and_plain_grep(fx, capsys):
    FakeClient.state["mailbox_result"] = {
        "deliveries": [{"delivery_id": "d:1", "message_id": "m:1", "sender": "@other", "kind": "private", "body": "the needle phrase", "state": "read", "created_at": "2026-08-24T00:00:01Z"}],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    cli.main(["--config", str(fx), "mail", "search", "needle"])
    out = capsys.readouterr().out
    assert "new" in out and "needle" in out
    # plain grep over the tree finds the known body string
    res = subprocess.run(["grep", "-r", "needle phrase", str(_mail_root())], capture_output=True, text=True, check=False)
    assert res.returncode == 0
    assert "the needle phrase" in res.stdout


def test_mail_ls_one_line_per_mail(fx, capsys):
    FakeClient.state["mailbox_result"] = {
        "deliveries": [{"delivery_id": "d:1", "message_id": "m:1", "sender": "@other", "kind": "private", "body": "listing me", "state": "read", "created_at": "2026-08-24T00:00:01Z"}],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])
    capsys.readouterr()
    cli.main(["--config", str(fx), "mail", "ls"])
    lines = [ln for ln in capsys.readouterr().out.strip().splitlines() if ln]
    assert len(lines) == 1
    assert "@other -> @me" in lines[0]
    assert "[private]" in lines[0]


def test_public_post_goes_through_outbox(fx):
    cli.main(["--config", str(fx), "public", "--post", "notice text"])
    payload = FakeClient.state["sends"][-1]
    assert payload["kind"] == "public"
    assert len(_files("sent")) == 1


def test_public_post_reply_to_goes_through_outbox(fx):
    """A threaded public reply rides the same outbox path as any other post,
    carrying the link and inheriting the target's key server-side."""
    FakeClient.state["send_result"] = {
        "message_id": "message:2",
        "recipient_count": 0,
        "correlation_id": "message:1",
    }
    cli.main(
        ["--config", str(fx), "public", "--post", "reply text",
         "--reply-to", "message:1"]
    )
    payload = FakeClient.state["sends"][-1]
    assert payload["kind"] == "public"
    assert payload["reply_to"] == "message:1"
    assert "correlation_id" not in payload  # the server derives it from the link
    # the sent copy carries the effective thread key, like a private reply
    from lloom.mailstore import MailStore

    mail = MailStore(_mail_root()).list("sent")[0]
    assert mail["reply_to"] == "message:1"
    assert mail["thread_key"] == "message:1"


def test_public_read_renders_thread_links(fx, capsys):
    """The board shows which post a reply answers — by the FULL id, because
    that is what a reader pastes into `--reply-to` (an 8-char prefix earned
    a live agent `invalid_reply_to`). Flat posts print unchanged."""
    root = "message:" + "a1" * 16
    reply = "message:" + "b2" * 16
    FakeClient.state["public_result"] = {
        "messages": [
            {"message_id": reply, "sender": "@aina", "body": "the dog, though",
             "reply_to": root, "correlation_id": root},
            {"message_id": root, "sender": "@marta", "body": "sant jordi plans?",
             "reply_to": None, "correlation_id": None},
        ]
    }
    cli.main(["--config", str(fx), "public"])
    out = capsys.readouterr().out.strip().splitlines()
    assert out[0] == f"[{reply}] @aina -> {root}: the dog, though"
    assert out[1] == f"[{root}] @marta: sant jordi plans?"


# -- P4: embedding backend (single embedder, no substitute vector) ------------


class _NoModelEmbedder:
    """Stand-in for Embedder when the real model cannot load."""

    def embed_or_none(self, text):
        return None


def test_no_model_no_flag_sends_without_embedding(fx, monkeypatch):
    import lloom.embed as embed_mod

    monkeypatch.setattr(cli, "_maybe_embed", _REAL_MAYBE_EMBED)
    monkeypatch.setattr(embed_mod, "Embedder", _NoModelEmbedder)
    cli.main(["--config", str(fx), "send", "--embed", "--to", "@other", "hi"])
    payload = FakeClient.state["sends"][-1]
    assert "embedding" not in payload  # never a substitute vector

    cli.main(["--config", str(fx), "broadcast", "fleet news"])
    payload = FakeClient.state["sends"][-1]
    assert "embedding" not in payload  # broadcasts fall back server-side


# -- review F1: retry reuses the sidecar vector; F2: tags presence ---------------


def test_embedded_send_sidecar_reused_verbatim_on_retry(fx, monkeypatch):
    """F1 (review): attempt 1 attached a model embedding and (unknown to the
    client) the server committed before the response was lost. A later
    retry — with the model now unavailable — must send the EXACT sidecar
    vector, not re-embed (or drop) it: a different digest under the same
    idempotency key is a permanent 409 idempotency_conflict."""
    vec = [0.25, 0.5, 0.75]
    monkeypatch.setattr(cli, "_maybe_embed", lambda body: vec)
    FakeClient.state["send_errors"] = [RuntimeError("connection refused")] * 5
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "send", "--embed", "--to", "@other", "durable vec"])

    assert len(_files("outbox")) == 1
    mail_id = _files("outbox")[0].stem
    sidecar = _mail_root() / "outbox" / f"{mail_id}.vec.json"
    assert json.loads(sidecar.read_text()) == {"embedding": vec, "backend": "model"}

    monkeypatch.setattr(cli, "_maybe_embed", lambda body: None)  # model gone
    FakeClient.state["send_errors"] = []
    cli.main(["--config", str(fx), "retry"])
    resent = FakeClient.state["sends"][-1]
    assert resent["embedding"] == vec  # sidecar vector, byte-identical
    assert len(_files("sent")) == 1
    # no sidecar survives the move to sent/
    assert not list((_mail_root() / "outbox").glob("*.vec.json"))
    assert not list((_mail_root() / "sent").glob("*.vec.json"))


def test_unembedded_send_creates_no_sidecar_and_retries_without_embedding(fx):
    FakeClient.state["send_errors"] = [RuntimeError("connection refused")] * 5
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "send", "--to", "@other", "plain"])
    assert len(_files("outbox")) == 1
    assert not list((_mail_root() / "outbox").glob("*.vec.json"))

    FakeClient.state["send_errors"] = []
    cli.main(["--config", str(fx), "retry"])
    assert "embedding" not in FakeClient.state["sends"][-1]
    assert len(_files("sent")) == 1


def test_dead_embedded_entry_drops_sidecar(fx, monkeypatch):
    """F1 (review): dead entries never resend, so their sidecar is deleted
    at dead-marking time (not left as outbox litter)."""
    monkeypatch.setattr(cli, "_maybe_embed", lambda body: [0.1])
    FakeClient.state["send_errors"] = [LloomError("recipient_not_found", "no such handle", 404)]
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "send", "--embed", "--to", "@ghost", "poison"])
    assert len(_files("outbox")) == 1
    assert "dead: true" in _files("outbox")[0].read_text()
    assert not list((_mail_root() / "outbox").glob("*.vec.json"))


def test_retry_payload_preserves_tags_presence(fx):
    """F2 (review): the retry payload must carry tags == [] when the original
    broadcast sent an explicit empty list, ["a"] when listed, and omit the
    key entirely when the original send had no tags — each combination is a
    different idempotency digest on the server."""
    from lloom.mailstore import MailStore

    store = MailStore(_mail_root())
    base = {"from": "@me", "to": None, "kind": "broadcast", "body": "b"}
    empty = store.enqueue({**base, "tags": [], "idempotency_key": "k-empty"})
    listed = store.enqueue({**base, "tags": ["a"], "idempotency_key": "k-a"})
    absent = store.enqueue({**base, "idempotency_key": "k-none"})

    assert cli._payload_from_mail(store.get(empty), store)["tags"] == []
    assert cli._payload_from_mail(store.get(listed), store)["tags"] == ["a"]
    assert "tags" not in cli._payload_from_mail(store.get(absent), store)


def test_legacy_home_store_db_also_migrated(fx, capsys, monkeypatch, tmp_path):
    """Review round 9: the pre-maildir client kept its outbox at
    ~/.lloom/store.db regardless of --config; the default-config user's
    queued sends must be discovered and drained, not lost."""
    home_db = tmp_path / "real-home-lloom" / "store.db"
    home_db.parent.mkdir(parents=True)
    conn = sqlite3.connect(home_db)
    conn.execute(
        "CREATE TABLE outbox (id TEXT PRIMARY KEY, created_at REAL, payload TEXT NOT NULL,"
        " status TEXT NOT NULL, attempts INTEGER DEFAULT 0, message_id TEXT, error TEXT)"
    )
    conn.execute(
        "INSERT INTO outbox (id, created_at, payload, status) VALUES (?, ?, ?, 'queued')",
        ("home-1", 1.0, json.dumps({"kind": "private", "to": "@other", "body": "from old home", "idempotency_key": "home-key"})),
    )
    conn.commit()
    conn.close()
    monkeypatch.setattr(cli, "legacy_home_store", lambda: home_db)

    cli.main(["--config", str(fx), "mail", "ls"])
    out = capsys.readouterr().out
    assert "from old home" in out
    assert len(_files("outbox")) == 1
    assert home_db.with_name("store.db.migrated").exists()


def test_legacy_geo_broadcast_fields_survive_migration(fx, capsys):
    """Review round 9: location/radius_km must round-trip the migration —
    dropping them changes the idempotency digest (409 on a committed send)
    or silently un-targets the broadcast."""
    state_dir = fx.parent / "state"
    state_dir.mkdir(exist_ok=True)
    db = state_dir / "store.db"
    conn = sqlite3.connect(db)
    conn.execute(
        "CREATE TABLE outbox (id TEXT PRIMARY KEY, created_at REAL, payload TEXT NOT NULL,"
        " status TEXT NOT NULL, attempts INTEGER DEFAULT 0, message_id TEXT, error TEXT)"
    )
    conn.execute(
        "INSERT INTO outbox (id, created_at, payload, status) VALUES (?, ?, ?, 'queued')",
        (
            "legacy-geo",
            1.0,
            json.dumps(
                {
                    "kind": "broadcast",
                    "to": None,
                    "body": "geo legacy",
                    "idempotency_key": "geo-key",
                    "location": {"lat": 41.38, "lng": 2.17},
                    "radius_km": 5.0,
                }
            ),
        ),
    )
    conn.commit()
    conn.close()

    cli.main(["--config", str(fx), "mail", "ls"])
    assert "geo legacy" in capsys.readouterr().out
    assert len(_files("outbox")) == 1
    from lloom.mailstore import MailStore

    store = MailStore(_mail_root())
    mail = store.get(_files("outbox")[0].stem)
    payload = cli._payload_from_mail(mail, store)
    assert payload["location"] == {"lat": 41.38, "lng": 2.17}
    assert payload["radius_km"] == 5.0


def test_legacy_expires_at_survives_migration(fx, capsys):
    """Review round 10: expires_at must round-trip the legacy migration —
    dropping it changes the idempotency digest of a queued public post."""
    state_dir = fx.parent / "state"
    state_dir.mkdir(exist_ok=True)
    db = state_dir / "store.db"
    conn = sqlite3.connect(db)
    conn.execute(
        "CREATE TABLE outbox (id TEXT PRIMARY KEY, created_at REAL, payload TEXT NOT NULL,"
        " status TEXT NOT NULL, attempts INTEGER DEFAULT 0, message_id TEXT, error TEXT)"
    )
    conn.execute(
        "INSERT INTO outbox (id, created_at, payload, status) VALUES (?, ?, ?, 'queued')",
        (
            "legacy-exp",
            1.0,
            json.dumps(
                {
                    "kind": "public",
                    "to": None,
                    "body": "expiring notice",
                    "idempotency_key": "exp-key",
                    "expires_at": "2026-09-01T00:00:00Z",
                }
            ),
        ),
    )
    conn.commit()
    conn.close()

    cli.main(["--config", str(fx), "mail", "ls"])
    assert "expiring notice" in capsys.readouterr().out
    assert len(_files("outbox")) == 1
    from lloom.mailstore import MailStore

    store = MailStore(_mail_root())
    mail = store.get(_files("outbox")[0].stem)
    payload = cli._payload_from_mail(mail, store)
    assert payload["expires_at"] == "2026-09-01T00:00:00Z"


# -- needs/offers card fields + broadcast intent -------------------------------


def test_register_with_needs_offers(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(
        ["--config", str(fx), "register", "@carded", "--needs", "rust review", "--offers", "python tooling"]
    )
    reg = FakeClient.state["register"]
    assert reg["needs"] == "rust review"
    assert reg["offers"] == "python tooling"


def test_update_with_needs_offers_attaches_local_vectors(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@carded2"])
    cli.main(["--config", str(fx), "update", "--needs", "ci wizard", "--offers", "docker help"])
    upd = FakeClient.state["updates"][-1]
    assert upd["needs"] == "ci wizard"
    assert upd["needs_embedding"] == [0.1, 0.2]  # local model stub
    assert upd["offers"] == "docker help"
    assert upd["offers_embedding"] == [0.1, 0.2]


def test_update_needs_offers_empty_clears_without_vector(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@carded3"])
    cli.main(["--config", str(fx), "update", "--needs", ""])
    upd = FakeClient.state["updates"][-1]
    assert upd["needs"] == ""
    assert "needs_embedding" not in upd


def test_update_geo_sets_location(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@geoset"])
    cli.main(["--config", str(fx), "update", "--geo", "41.4036,2.1560"])
    upd = FakeClient.state["updates"][-1]
    assert upd["location"] == {"lat": 41.4036, "lng": 2.1560}


def test_update_geo_empty_string_clears_location(fx):
    """`--geo ""` mirrors `--needs ""`: the key must be PRESENT and null, or
    the server reads it as "leave the location alone"."""
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@geoclear"])
    cli.main(["--config", str(fx), "update", "--geo", ""])
    upd = FakeClient.state["updates"][-1]
    assert "location" in upd
    assert upd["location"] is None


def test_update_without_geo_omits_location(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@geokeep"])
    cli.main(["--config", str(fx), "update", "--description", "moved on"])
    upd = FakeClient.state["updates"][-1]
    assert "location" not in upd


def test_parse_geo_accepts_a_point_and_empty(fx):
    assert cli._parse_geo("41.4036,2.1560") == {"lat": 41.4036, "lng": 2.1560}
    assert cli._parse_geo(" 41.4036 , 2.1560 ") == {"lat": 41.4036, "lng": 2.1560}
    assert cli._parse_geo("") is None
    assert cli._parse_geo(None) is None


def test_parse_geo_rejects_out_of_range(fx):
    """A swapped 'lng,lat' pair is the common mistake: Barcelona's 2.17 is a
    fine latitude, but its 41.39 is caught here rather than by the server."""
    for bad in ("91,0", "-91,0", "0,181", "0,-181"):
        with pytest.raises(SystemExit) as exc:
            cli._parse_geo(bad)
        assert "within" in str(exc.value)


def test_parse_geo_rejects_malformed(fx):
    for bad in ("41.4", "41.4,2.15,3", "abc,1", "41.4;2.15"):
        with pytest.raises(SystemExit):
            cli._parse_geo(bad)


def test_register_geo_reaches_the_payload(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@georeg", "--geo", "41.3797,2.1686"])
    assert FakeClient.state["register"]["location"] == {"lat": 41.3797, "lng": 2.1686}


def test_broadcast_geo_and_radius_reach_the_payload(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@geocast"])
    cli.main([
        "--config", str(fx), "broadcast",
        "--geo", "41.3797,2.1686", "--radius-km", "2", "a flat in Raval",
    ])
    payload = FakeClient.state["sends"][-1]
    assert payload["location"] == {"lat": 41.3797, "lng": 2.1686}
    assert payload["radius_km"] == 2.0


def test_broadcast_geo_without_radius_exits(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@geohalf"])
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "broadcast", "--geo", "41.3797,2.1686", "no radius"])


def test_broadcast_intent_flag_in_payload(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@caster"])
    cli.main(["--config", str(fx), "broadcast", "--intent", "seeking", "need ci help"])
    payload = FakeClient.state["sends"][-1]
    assert payload["intent"] == "seeking"


def test_broadcast_no_intent_by_default(fx):
    import os

    os.environ["LLOOM_PASSWORD"] = "supersecret"
    cli.main(["--config", str(fx), "register", "@caster2"])
    cli.main(["--config", str(fx), "broadcast", "plain announcement"])
    payload = FakeClient.state["sends"][-1]
    assert "intent" not in payload


# -- handle collisions + locally generated passwords -------------------------


_TAKEN_ERROR = LloomError(
    "handle_taken",
    "handle marta_coll already taken",
    409,
    details={"suggestions": ["coll_marta", "m_coll", "marta_coll_2"]},
)


def test_register_handle_taken_prints_free_alternatives(fx, capsys, monkeypatch):
    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    FakeClient.state["register_error"] = _TAKEN_ERROR
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "register", "@marta_coll"])
    assert ei.value.code == 1
    err = capsys.readouterr().err
    # the conformance gate keys on this literal appearing in stderr
    assert "handle_taken" in err
    assert "free alternatives: @coll_marta, @m_coll, @marta_coll_2" in err


def test_handle_check_available(fx, capsys):
    FakeClient.state["check_handle_result"] = {
        "handle": "marta_coll", "available": True, "suggestions": []
    }
    cli.main(["--config", str(fx), "handle-check", "@marta_coll"])
    assert "@marta_coll is available" in capsys.readouterr().out
    assert FakeClient.state["check_handle"] == "@marta_coll"


def test_handle_check_taken_exits_1_with_alternatives(fx, capsys):
    FakeClient.state["check_handle_result"] = {
        "handle": "marta_coll", "available": False, "suggestions": ["coll_marta", "marta_coll_2"],
    }
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "handle-check", "@marta_coll"])
    assert ei.value.code == 1
    err = capsys.readouterr().err
    assert "handle_taken" in err
    assert "free alternatives: @coll_marta, @marta_coll_2" in err


def test_password_auto_generates_locally_and_never_prints_it(fx, capsys):
    cli.main(["--config", str(fx), "register", "@newbie", "--password-auto"])
    password = FakeClient.state["register"]["password"]
    assert len(password) >= 16

    out = capsys.readouterr()
    # the whole point: the secret must not reach the terminal, so an agent
    # driving the CLI never sees it
    assert password not in out.out
    assert password not in out.err
    assert "registered @newbie" in out.out
    assert str(fx) in out.out and "password" in out.out

    saved = json.loads(fx.read_text())
    assert saved["password"] == password
    assert saved["api_key"] == "llm_new"


def test_password_auto_writes_a_private_config_file(fx):
    import stat

    cli.main(["--config", str(fx), "register", "@newbie", "--password-auto"])
    assert stat.S_IMODE(fx.stat().st_mode) == 0o600


def test_generated_passwords_are_unique():
    assert cli._generate_password() != cli._generate_password()


def test_password_auto_conflicts_with_password_stdin(fx):
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "register", "@newbie", "--password-auto", "--password-stdin"])
    assert ei.value.code == 2


def test_supplied_password_is_not_persisted(fx, monkeypatch):
    """Only auto-generated passwords are stored; a user's own stays theirs."""
    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    cli.main(["--config", str(fx), "register", "@newbie"])
    assert "password" not in json.loads(fx.read_text())


def test_login_and_rotate_reuse_the_stored_password(fx, capsys):
    cli.main(["--config", str(fx), "register", "@newbie", "--password-auto"])
    generated = FakeClient.state["register"]["password"]

    # no flag, no env, no TTY: login still works off the stored secret
    cli.main(["--config", str(fx), "login", "@newbie"])
    assert FakeClient.state["login"]["password"] == generated

    cli.main(["--config", str(fx), "rotate"])
    assert FakeClient.state["rotate"]["password"] == generated
    assert generated not in capsys.readouterr().out


def test_explicit_password_sources_still_outrank_the_stored_one(fx, monkeypatch):
    cli.main(["--config", str(fx), "register", "@newbie", "--password-auto"])
    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    cli.main(["--config", str(fx), "login", "@newbie"])
    assert FakeClient.state["login"]["password"] == "supersecret"


def test_config_show_redacts_the_stored_password(fx, capsys):
    cli.main(["--config", str(fx), "register", "@newbie", "--password-auto"])
    generated = FakeClient.state["register"]["password"]
    capsys.readouterr()

    cli.main(["--config", str(fx), "config", "show"])
    out = capsys.readouterr().out
    assert generated not in out
    assert '"password": "****"' in out


def test_stored_password_is_not_sent_to_a_different_handle(fx):
    cli.main(["--config", str(fx), "register", "@newbie", "--password-auto"])
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "login", "@someone_else"])
    assert ei.value.code == 2


def test_stored_password_matches_regardless_of_at_sign_and_case(fx):
    cli.main(["--config", str(fx), "register", "@newbie", "--password-auto"])
    generated = FakeClient.state["register"]["password"]
    cli.main(["--config", str(fx), "login", "NewBie"])
    assert FakeClient.state["login"]["password"] == generated


# -- P1.4: one id space at the CLI -------------------------------------------


def _poll_one(fx, delivery_id="delivery:zzqf86feqm806d54tr2o", message_id="message:" + "ab" * 16, body="the mail"):
    FakeClient.state["mailbox_result"] = {
        "deliveries": [{
            "delivery_id": delivery_id, "message_id": message_id, "sender": "@other",
            "kind": "private", "body": body, "state": "read",
            "created_at": "2026-08-24T00:00:01Z",
        }],
        "next_cursor": None,
    }
    cli.main(["--config", str(fx), "poll"])


@pytest.mark.parametrize(
    "token",
    [
        "delivery:zzqf86feqm806d54tr2o",  # straight off `lloom poll`
        "zzqf86feqm806d54tr2o",           # …with the record prefix stripped
        "message:" + "ab" * 16,           # the id a reply threads to
        "ab" * 16,
        "abababab",                       # the 8-char display prefix
    ],
)
def test_mail_read_accepts_any_of_the_three_ids(fx, capsys, token):
    """Every real CLI failure in the last scenario run was `lloom mail read`
    handed a delivery or message id instead of the local mail id."""
    _poll_one(fx)
    capsys.readouterr()
    cli.main(["--config", str(fx), "mail", "read", token])
    assert "the mail" in capsys.readouterr().out
    assert len(_files("read")) == 1  # reading IS filing, whichever id was used


def test_mail_read_still_takes_the_local_mail_id_prefix(fx, capsys):
    _poll_one(fx)
    mail_id = _files("new")[0].stem
    capsys.readouterr()
    cli.main(["--config", str(fx), "mail", "read", mail_id[:8]])
    assert "the mail" in capsys.readouterr().out


def test_mail_read_unknown_id_exits_1(fx, capsys):
    _poll_one(fx)
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "mail", "read", "deadbeef"])
    assert ei.value.code == 1
    assert "no mail with id prefix" in capsys.readouterr().err


@pytest.mark.parametrize("token", ["message:" + "ab" * 16, "abababab", "zzqf86fe"])
def test_ack_accepts_a_message_or_mail_id_and_acks_the_delivery(fx, token):
    """`lloom ack` is documented with a delivery id; the id an agent holds is
    as often the message id it just replied to."""
    _poll_one(fx)
    cli.main(["--config", str(fx), "ack", token])
    assert FakeClient.state["acks"] == ["delivery:zzqf86feqm806d54tr2o"]  # resolved locally
    assert _files("new") == []
    assert "acked_at: null" not in _files("read")[0].read_text()


def test_ack_by_local_mail_id(fx):
    _poll_one(fx)
    mail_id = _files("new")[0].stem
    cli.main(["--config", str(fx), "ack", mail_id[:8]])
    assert FakeClient.state["acks"] == ["delivery:zzqf86feqm806d54tr2o"]


def test_ack_of_an_id_the_maildir_never_saw_goes_to_the_server_verbatim(fx):
    """A cleared maildir (or `poll --json` piped into ack) must still work:
    an unresolvable id is the server's call, not the client's."""
    cli.main(["--config", str(fx), "ack", "delivery:not-in-the-store"])
    assert FakeClient.state["acks"] == ["delivery:not-in-the-store"]


@pytest.mark.parametrize(
    "token",
    [
        "delivery:zzqf86feqm806d54tr2o",  # straight off `lloom poll`
        "zzqf86feqm806d54tr2o",
        "abababab",                       # the 8-char local display prefix
    ],
)
def test_rate_resolves_any_local_id_to_the_delivery(fx, token):
    """A rating names a delivery; the id an agent is holding is often not one."""
    _poll_one(fx)
    cli.main(["--config", str(fx), "rate", token, "helpful", "--note", "worked"])
    assert FakeClient.state["feedback"] == [
        {
            "verdict": "helpful",
            "delivery_id": "delivery:zzqf86feqm806d54tr2o",
            "message_id": None,
            "note": "worked",
            "block": None,
        }
    ]


def test_report_blocks_by_default_and_no_block_opts_out(fx):
    _poll_one(fx)
    cli.main(["--config", str(fx), "report", "delivery:zzqf86feqm806d54tr2o", "spam"])
    cli.main(["--config", str(fx), "report", "delivery:zzqf86feqm806d54tr2o", "spam", "--no-block"])
    assert [c["block"] for c in FakeClient.state["feedback"]] == [True, False]
    assert [c["verdict"] for c in FakeClient.state["feedback"]] == ["spam", "spam"]


def test_a_message_id_reports_a_public_post_by_message(fx):
    """The one form that is NOT resolved to a delivery: the board has none."""
    cli.main(["--config", str(fx), "report", "message:" + "cd" * 16, "impersonation"])
    [call] = FakeClient.state["feedback"]
    assert call["message_id"] == "message:" + "cd" * 16
    assert call["delivery_id"] is None


def test_a_zero_weight_verdict_says_why_it_counted_for_nothing(fx, capsys):
    """`weight_applied: 0` reads as a failure unless the CLI explains it."""
    _poll_one(fx)
    FakeClient.state["feedback_weight"] = 0
    cli.main(["--config", str(fx), "rate", "delivery:zzqf86feqm806d54tr2o", "not_helpful"])
    assert "counted for nothing" in capsys.readouterr().err


def test_a_feedback_error_exits_1_with_the_reason(fx, capsys):
    FakeClient.state["feedback_errors"] = [
        LloomError("feedback_duplicate", "already filed", 409)
    ]
    _poll_one(fx)
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "rate", "delivery:zzqf86feqm806d54tr2o", "helpful"])
    assert ei.value.code == 1
    assert "already filed" in capsys.readouterr().err


def test_block_takes_a_handle_and_lists_when_given_none(fx):
    cli.main(["--config", str(fx), "block", "@spammer"])
    assert FakeClient.state["blocks"] == ["@spammer"]
    cli.main(["--config", str(fx), "unblock", "@spammer"])
    assert FakeClient.state["unblocks"] == ["@spammer"]
    cli.main(["--config", str(fx), "block"])
    assert FakeClient.state["blocks_listed"] == {"limit": 50}


def test_ack_of_an_ambiguous_prefix_exits_1_without_acking(fx, capsys):
    _poll_one(fx, delivery_id="delivery:d1", message_id="message:beef0001", body="one")
    _poll_one(fx, delivery_id="delivery:d2", message_id="message:beef0002", body="two")
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "ack", "beef"])
    assert ei.value.code == 1
    assert "ambiguous" in capsys.readouterr().err
    assert FakeClient.state.get("acks") is None  # never acked a guess


@pytest.mark.parametrize(
    ("given", "sent"),
    [
        ("marc_pujol", "@marc_pujol"),   # the shape that failed recipient_not_found
        ("@marc_pujol", "@marc_pujol"),  # unchanged
        ("Marc_Pujol", "@Marc_Pujol"),   # the server normalizes case itself
        ("agent:jj6756kk1dh011dyt5hg", "agent:jj6756kk1dh011dyt5hg"),  # an agent id
        ("no", "no"),                    # too short to be a handle: left alone
        ("marc.pujol", "marc.pujol"),    # not the handle grammar: left alone
    ],
)
def test_send_to_forgives_a_missing_at_sign(fx, given, sent):
    cli.main(["--config", str(fx), "send", "--to", given, "hi"])
    assert FakeClient.state["sends"][-1]["to"] == sent


# -- P1.5: duplicate-send suppression ----------------------------------------


def test_identical_send_inside_the_window_replays_instead_of_duplicating(fx, capsys, monkeypatch):
    """4 of 109 private messages in a scenario run were exact duplicates: a
    fresh uuid4 per send left the server's replay protection disarmed."""
    monkeypatch.setattr(cli.time, "time", lambda: 1_700_000_000.0)
    cli.main(["--config", str(fx), "send", "--to", "@other", "same body"])
    first_out = json.loads(capsys.readouterr().out)
    cli.main(["--config", str(fx), "send", "--to", "@other", "same body"])
    second_out = json.loads(capsys.readouterr().out)
    first, second = FakeClient.state["sends"]
    assert first["idempotency_key"] == second["idempotency_key"]
    assert first_out.get("replayed") is not True  # the first one was delivered
    assert second_out["replayed"] is True  # the second was not
    assert len(_files("sent")) == 1  # one message on the wire, one file locally


def test_duplicate_send_replays_on_the_real_clock_too(fx, capsys):
    """The same thing without a frozen clock: two identical sends a
    millisecond apart share the window they were made in."""
    cli.main(["--config", str(fx), "send", "--to", "@other", "same body"])
    capsys.readouterr()
    cli.main(["--config", str(fx), "send", "--to", "@other", "same body"])
    assert json.loads(capsys.readouterr().out)["replayed"] is True


def test_force_opts_out_with_a_fresh_key(fx):
    cli.main(["--config", str(fx), "send", "--to", "@other", "same body"])
    cli.main(["--config", str(fx), "send", "--force", "--to", "@other", "same body"])
    first, second = FakeClient.state["sends"]
    assert first["idempotency_key"] != second["idempotency_key"]
    assert len(_files("sent")) == 2  # a re-send that was meant


def test_a_different_message_is_never_deduped(fx):
    cli.main(["--config", str(fx), "send", "--to", "@other", "one"])
    cli.main(["--config", str(fx), "send", "--to", "@other", "two"])
    cli.main(["--config", str(fx), "send", "--to", "@someone", "one"])
    keys = [p["idempotency_key"] for p in FakeClient.state["sends"]]
    assert len(set(keys)) == 3
    assert len(_files("sent")) == 3


def test_broadcast_and_public_dedupe_too(fx):
    cli.main(["--config", str(fx), "broadcast", "hello world"])
    cli.main(["--config", str(fx), "broadcast", "hello world"])
    cli.main(["--config", str(fx), "public", "--post", "notice text"])
    cli.main(["--config", str(fx), "public", "--post", "notice text"])
    keys = [p["idempotency_key"] for p in FakeClient.state["sends"]]
    assert keys[0] == keys[1]
    assert keys[2] == keys[3]
    assert len(_files("sent")) == 2


def test_dedupe_key_covers_every_field_the_server_digests():
    """A key that collided across two DIFFERENT requests would come back a
    permanent 409 idempotency_conflict and park the send dead in the outbox,
    so the key hashes the same fields the server's digest does."""
    base = {"kind": "private", "to": "@other", "body": "hi"}
    now = 1_700_000_000.0
    key = cli._dedupe_key("@me", base, now=now)
    assert key == cli._dedupe_key("@me", dict(base), now=now)  # deterministic
    assert key != cli._dedupe_key("@you", base, now=now)  # sender
    for field, value in (
        ("to", "@third"),
        ("body", "different"),
        ("kind", "broadcast"),
        ("reply_to", "message:1"),
        ("correlation_id", "job-42"),
        ("embedding", [0.1, 0.2]),
        ("tags", ["ops"]),
        ("intent", "seeking"),
        ("location", {"lat": 41.5, "lng": 2.1}),
        ("radius_km", 5.0),
        ("expires_at", "2026-09-01T00:00:00Z"),
    ):
        assert key != cli._dedupe_key("@me", {**base, field: value}, now=now), field


def test_dedupe_key_expires_with_the_window():
    base = {"kind": "private", "to": "@other", "body": "hi"}
    start = float(cli.DEDUPE_WINDOW_S * 1_000_000)  # a bucket boundary
    key = cli._dedupe_key("@me", base, now=start)
    assert cli._dedupe_key("@me", base, now=start + cli.DEDUPE_WINDOW_S - 1) == key
    assert cli._dedupe_key("@me", base, now=start + cli.DEDUPE_WINDOW_S) != key


def test_routing_prints_the_stored_decision(fx, capsys):
    """`lloom routing <id>` reads a broadcast's decision back from the server.

    The 201 that reported `recipients`/`rejected` is long gone by the time an
    agent wonders why a broadcast landed where it did, and selection ran in an
    in-memory index that no longer holds it.
    """
    record = {
        "message_id": "message:abc",
        "intent": "seeking",
        "threshold": 0.4,
        "floor": 0.4,
        "margin": 0.15,
        "cutoff": 0.55,
        "top_n": 5,
        "candidates": 9,
        "truncated": False,
        "min_accepted": 0.69,
        "max_rejected": 0.55,
        "accepted": [{"agent_id": "agent:9", "score": 0.69}],
        "rejected": [{"agent_id": "agent:8", "score": 0.55, "reason": "outside_margin"}],
        "rejected_counts": {"outside_margin": 8, "excluded_sender": 1},
    }
    FakeClient.state["routing_result"] = record
    cli.main(["--config", str(fx), "routing", "message:abc"])

    assert FakeClient.state["routing_calls"] == ["message:abc"]
    assert json.loads(capsys.readouterr().out) == record


def test_routing_passes_a_bare_id_through_untouched(fx, capsys):
    """The server normalizes a bare id; the client does not second-guess it."""
    FakeClient.state["routing_result"] = {"message_id": "message:" + "a" * 32}
    cli.main(["--config", str(fx), "routing", "a" * 32])
    assert FakeClient.state["routing_calls"] == ["a" * 32]


def test_routing_reports_a_server_error_without_a_traceback(fx, capsys):
    FakeClient.state["routing_error"] = LloomError(
        "routing_not_found", "a private message carries no routing record", 404
    )
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "routing", "message:abc"])



# -- P3.1 / P3.2: thread annotation, thread view, correlation stamping --------


def _poll_thread(fx, deliveries):
    FakeClient.state["mailbox_result"] = {"deliveries": deliveries, "next_cursor": None}
    cli.main(["--config", str(fx), "poll"])


def _delivery(**over):
    base = {
        "delivery_id": "delivery:d1", "message_id": "message:m1", "sender": "@marc",
        "kind": "private", "body": "shall we say four?", "state": "read",
        "created_at": "2026-08-30T16:00:00Z", "reply_to": None,
        "correlation_id": "message:m1", "thread_key": "message:m1",
        "superseded_by": None,
    }
    base.update(over)
    return base


def test_poll_files_and_prints_the_supersession_hint(fx, capsys):
    """The one bit that would have prevented 20 of 109 stale replies: there
    is something newer from this person in this thread."""
    _poll_thread(fx, [
        _delivery(superseded_by="message:m2"),
        _delivery(delivery_id="delivery:d2", message_id="message:m2",
                  body="actually, Saturday at ten", reply_to="message:m1",
                  created_at="2026-08-30T16:05:00Z"),
    ])
    out = capsys.readouterr().out
    assert "superseded_by=message:m2" in out
    assert "answer that one" in out
    assert "thread=message:m1" in out

    stale = [p for p in _files("new") if "superseded_by: message:m2" in p.read_text()]
    assert len(stale) == 1
    assert "thread_key: message:m1" in stale[0].read_text()


def test_a_re_served_delivery_refreshes_a_stale_annotation(fx):
    """The annotation is derived per poll and is NOT in the content hash, so
    the second filing lands on the first file: it must overwrite, not keep
    the "nothing newer" the first poll wrote."""
    _poll_thread(fx, [_delivery()])
    assert len(_files("new")) == 1
    assert "superseded_by: null" in _files("new")[0].read_text()

    _poll_thread(fx, [_delivery(superseded_by="message:m2")])
    assert len(_files("new")) == 1  # same content, same file
    assert "superseded_by: message:m2" in _files("new")[0].read_text()


def test_mail_thread_prints_the_whole_conversation(fx, capsys):
    """The view an agent needs before answering — both halves, in order."""
    FakeClient.state["send_result"] = {
        "message_id": "message:m2", "recipient_count": 1, "correlation_id": "message:m1",
    }
    _poll_thread(fx, [_delivery()])  # inbound: correlation_id straight off the wire
    cli.main(["--config", str(fx), "send", "--to", "@marc",
              "--reply-to", "message:m1", "ten at the cafe?"])
    capsys.readouterr()

    cli.main(["--config", str(fx), "mail", "thread", "m1"])
    out = capsys.readouterr().out
    assert "thread message:m1 — 2 messages, oldest first" in out
    assert out.index("shall we say four?") < out.index("ten at the cafe?")
    assert "@marc -> @me" in out and "@me -> @marc" in out


@pytest.mark.parametrize("token", ["delivery:d1", "message:m1", "m1", "d1"])
def test_mail_thread_accepts_any_of_the_three_ids(fx, capsys, token):
    _poll_thread(fx, [_delivery()])
    cli.main(["--config", str(fx), "mail", "thread", token])
    assert "shall we say four?" in capsys.readouterr().out


def test_mail_thread_unknown_id_exits_1(fx, capsys):
    _poll_thread(fx, [_delivery()])
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "mail", "thread", "deadbeef"])
    assert ei.value.code == 1
    assert "no mail with id prefix" in capsys.readouterr().err


def test_mail_thread_on_an_unsent_outbox_entry_exits_1(fx, capsys):
    """Nothing has a server id yet, so there is no thread to print."""
    FakeClient.state["send_errors"] = [LloomError("http_error", "server down", 0)] * 5
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "send", "--to", "@marc", "queued"])
    capsys.readouterr()
    mail_id = _files("outbox")[0].stem
    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "mail", "thread", mail_id[:8]])
    assert ei.value.code == 1
    assert "not part of a thread yet" in capsys.readouterr().err


def test_send_stamps_the_servers_thread_key_on_the_sent_copy(fx):
    """The server derives the key when the caller supplies none; without
    filing it back, the agent's own half of the exchange carries none."""
    FakeClient.state["send_result"] = {
        "message_id": "message:m9", "recipient_count": 1, "correlation_id": "message:m9",
    }
    cli.main(["--config", str(fx), "send", "--to", "@marc", "cold open"])
    text = _files("sent")[0].read_text()
    assert "thread_key: message:m9" in text
    assert "message_id: message:m9" in text
    # NOT into correlation_id: that field is the request a retry replays
    assert "correlation_id: null" in text
    assert _files("outbox") == []


def test_a_re_filed_sent_entry_still_replays_after_the_stamp(fx, capsys):
    """The stamp must not change the request. A sent/ file put back in the
    outbox — the shape the durability suite exercises — has to replay the
    identical payload, or it is a permanent 409 idempotency_conflict on a
    send the server already committed."""
    FakeClient.state["send_result"] = {
        "message_id": "message:m8", "recipient_count": 1, "correlation_id": "message:m8",
    }
    cli.main(["--config", str(fx), "send", "--to", "@marc", "committed once"])
    capsys.readouterr()
    sent = _files("sent")[0]
    sent.rename(_mail_root() / "outbox" / sent.name)

    cli.main(["--config", str(fx), "retry"])
    first, second = FakeClient.state["sends"]
    assert second == first  # byte-identical request, so the server replays it
    assert _files("outbox") == []


def test_the_thread_key_is_never_stamped_on_a_pending_outbox_entry(fx, capsys):
    """A key on an entry the server has not accepted would be replayed as
    part of the request it never received."""
    FakeClient.state["send_errors"] = [LloomError("http_error", "boom", 0)] * 5
    with pytest.raises(SystemExit):
        cli.main(["--config", str(fx), "send", "--to", "@marc", "will retry"])
    capsys.readouterr()
    outbox_text = _files("outbox")[0].read_text()
    assert "correlation_id: null" in outbox_text
    assert "thread_key: null" in outbox_text

    FakeClient.state["send_result"] = {
        "message_id": "message:m7", "recipient_count": 1, "correlation_id": "message:m7",
    }
    cli.main(["--config", str(fx), "retry"])
    assert FakeClient.state["sends"][-1]["correlation_id"] is None  # replayed as sent
    assert "thread_key: message:m7" in _files("sent")[0].read_text()


# -- P4.2: an empty poll says something ---------------------------------------


def test_empty_poll_reports_the_silence_and_the_alternative(fx, capsys):
    """509 polls for 206 deliveries: a model handed an empty screen re-polls.
    An empty poll must print a fact and a next step instead of nothing."""
    cli.main(["--config", str(fx), "poll"])
    out = capsys.readouterr().out
    assert "nothing new" in out
    assert "--wait" in out


def test_empty_poll_dates_the_silence_from_the_last_delivery(fx, capsys):
    """`since <time>` is the moment this mailbox last produced mail — the
    cursor file's mtime, written only after a poll's deliveries are filed."""
    FakeClient.state["mailbox_result"] = {
        "deliveries": [_delivery()], "next_cursor": "CURSOR_1",
    }
    cli.main(["--config", str(fx), "poll"])
    capsys.readouterr()

    FakeClient.state["mailbox_result"] = {"deliveries": [], "next_cursor": None}
    cli.main(["--config", str(fx), "poll"])
    out = capsys.readouterr().out
    stamp = time.strftime(
        "%Y-%m-%d %H:%M",
        time.localtime((fx.parent / "state" / fx.name / "cursor.txt").stat().st_mtime),
    )
    assert f"nothing new since {stamp}" in out


def test_an_empty_long_poll_does_not_suggest_the_flag_it_just_used(fx, capsys):
    cli.main(["--config", str(fx), "poll", "--wait", "5"])
    out = capsys.readouterr().out
    assert "nothing new" in out
    assert "--wait" not in out


def test_empty_poll_json_stays_machine_clean(fx, capsys):
    cli.main(["--config", str(fx), "poll", "--json"])
    assert json.loads(capsys.readouterr().out.strip()) == {
        "deliveries": [], "next_cursor": None,
    }


# -- B4: the registration proof-of-work, solved without the user noticing ----


def _challenge_error(difficulty: int = 8, nonce: str = "nonce-abc", alg: str = "sha256-prefix"):
    return LloomError(
        "challenge_required",
        "registration requires a proof-of-work challenge; solve it and retry",
        428,
        details={
            "nonce": nonce,
            "difficulty": difficulty,
            "alg": alg,
            "expires_in": 120,
            "reason": "required",
        },
    )


def test_register_solves_a_428_challenge_and_succeeds_on_the_retry(fx, capsys, monkeypatch):
    """The whole point of B4 on the client: a hub under pressure costs the
    user a fraction of a second and one line of explanation, not an error."""
    import hashlib

    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    FakeClient.state["register_errors"] = [_challenge_error(difficulty=8)]

    cli.main(["--config", str(fx), "register", "@pow_user"])

    calls = FakeClient.state["registers"]
    assert len(calls) == 2  # the challenged attempt, then the solved one
    assert calls[0]["challenge"] is None
    answer = calls[1]["challenge"]
    digest = hashlib.sha256(b"nonce-abc" + str(answer["counter"]).encode()).digest()
    assert int.from_bytes(digest, "big") >> (256 - 8) == 0  # a REAL solution
    assert answer["nonce"] == "nonce-abc"

    captured = capsys.readouterr()
    assert "solving registration challenge (difficulty 8)" in captured.err
    assert "registered @pow_user" in captured.out
    assert json.loads(fx.read_text())["api_key"] == "llm_new"


def test_register_challenged_twice_fails_clearly(fx, capsys, monkeypatch):
    """One retry, not a grind loop: a hub that challenges the solved attempt
    too is busy enough that the honest answer is 'come back in a minute'."""
    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    FakeClient.state["register_errors"] = [_challenge_error(), _challenge_error()]

    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "register", "@pow_twice"])

    assert ei.value.code == 1
    assert len(FakeClient.state["registers"]) == 2  # tried twice, never a third
    err = capsys.readouterr().err
    assert "challenged this registration twice" in err
    assert "Try again in a minute" in err
    # a failed registration leaves the existing config untouched
    assert json.loads(fx.read_text())["api_key"] == "llm_key"


def test_register_refuses_a_challenge_it_cannot_answer(fx, capsys, monkeypatch):
    """An algorithm this client does not implement is a stop, not a guess."""
    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    FakeClient.state["register_errors"] = [_challenge_error(alg="scrypt-of-the-future")]

    with pytest.raises(SystemExit) as ei:
        cli.main(["--config", str(fx), "register", "@pow_alien"])

    assert ei.value.code == 1
    assert len(FakeClient.state["registers"]) == 1  # never sent a bogus answer
    assert "unsupported challenge algorithm" in capsys.readouterr().err


def test_an_unchallenged_registration_sends_no_challenge_field(fx, monkeypatch):
    """At rest the wire looks exactly as it did before B4."""
    monkeypatch.setenv("LLOOM_PASSWORD", "supersecret")
    cli.main(["--config", str(fx), "register", "@pow_quiet"])
    assert FakeClient.state["registers"] == [FakeClient.state["register"]]
    assert FakeClient.state["register"]["challenge"] is None


# -- whoami / reputation: the standing an agent reads about itself -------------
#
# Both print their JSON on stdout and a human summary on stderr, so
# `lloom whoami | jq` keeps working while an agent reading the terminal is
# told the four things it cannot discover from a 60-second window: its tier,
# its score, the gap to the next tier, and what is left of today's quotas.

WHOAMI_FULL = {
    "agent_id": "agent:1",
    "handle": "@me",
    "status": "active",
    "tier": "T0",
    "score": 7.5,
    "score_band": "neutral",
    "next_tier": {
        "name": "T1",
        "missing": [
            {"gate": "score", "have": 7.5, "need": 15.0},
            {"gate": "counterparties", "have": 2, "need": 5.0},
        ],
    },
    "restricted": None,
    "moderation": None,
    "quota": {
        "remaining": {"cold_opens": 3, "broadcasts": 4, "public_posts": None, "feedback": 5},
        "used": {"cold_opens": 5, "broadcasts": 0, "public_posts": 0, "feedback": 0},
        "resets_at": "2026-09-07T00:00:00Z",
    },
}


def test_whoami_prints_tier_score_gap_and_quota(fx, capsys):
    FakeClient.state["whoami_result"] = WHOAMI_FULL
    cli.main(["--config", str(fx), "whoami"])
    out, err = capsys.readouterr()

    # stdout stays the raw response, so a pipe into jq is unaffected
    assert json.loads(out)["tier"] == "T0"

    assert "tier T0" in err
    assert "score 7.5 (neutral)" in err
    # the gap is expressed as what is still MISSING, not as the thresholds --
    # "7.5 score" is something an agent can act on, "need 15" is arithmetic
    assert "next: T1 in 7.5 score, 3 counterparties" in err
    # a null quota is unlimited and is left out rather than printed as "None"
    assert "cold_opens 3" in err and "broadcasts 4" in err
    assert "public_posts" not in err
    assert "resets 2026-09-07T00:00:00Z" in err


def test_whoami_says_when_the_next_tier_is_already_earned(fx, capsys):
    """An empty `missing` means the next batch promotes you. An agent that
    read only "T0" could not tell that from "nothing is happening"."""
    FakeClient.state["whoami_result"] = {
        **WHOAMI_FULL, "next_tier": {"name": "T1", "missing": []}
    }
    cli.main(["--config", str(fx), "whoami"])
    assert "next: T1 (qualified)" in capsys.readouterr().err


def test_whoami_spells_out_a_restriction_and_how_to_leave_it(fx, capsys):
    FakeClient.state["whoami_result"] = {
        **WHOAMI_FULL,
        "tier": "R",
        "score": -24.0,
        "next_tier": None,
        "restricted": {
            "reason": "score",
            "until": "2026-09-08T00:00:00Z",
            "recover": "score >= -5 and at least 48 h in R",
        },
    }
    cli.main(["--config", str(fx), "whoami"])
    err = capsys.readouterr().err
    assert "RESTRICTED (score) until 2026-09-08T00:00:00Z" in err
    # the load-bearing half of the state: what still works
    assert "replies in" in err and "existing threads still work" in err
    assert "Recover: score >= -5 and at least 48 h in R" in err


def test_whoami_reports_a_moderator_verdict(fx, capsys):
    """A muted agent can still call whoami, and this is the only place it
    finds out -- otherwise it discovers the mute as a refused send."""
    FakeClient.state["whoami_result"] = {
        **WHOAMI_FULL,
        "moderation": {"state": "muted", "until": "2026-09-09T00:00:00Z"},
    }
    cli.main(["--config", str(fx), "whoami"])
    err = capsys.readouterr().err
    assert "MODERATED: muted until 2026-09-09T00:00:00Z" in err
    assert "every send and profile update is refused" in err


def test_whoami_on_a_hub_without_tiers_prints_no_summary(fx, capsys):
    """No `tier` in the response means a hub that predates them: print the
    JSON and nothing else rather than inventing a status line."""
    cli.main(["--config", str(fx), "whoami"])
    out, err = capsys.readouterr()
    assert json.loads(out)["handle"] == "@me"
    assert err.strip() == ""


REPUTATION_FULL = {
    "agent_id": "agent:1",
    "tier": "T0",
    "score": 7.5,
    "score_band": "neutral",
    "components": {"positive": 9.0, "negative": 3.5, "tenure": 2.0},
    "tenure_days": 4,
    "counterparties": 2,
    "profile_bonus": True,
    "next_tier": {"name": "T1", "missing": [{"gate": "score", "have": 7.5, "need": 15.0}]},
    "restricted": None,
    "totals": {"ack": 4.0, "reply_cold_open": 3.0, "duplicate_body": -2.0},
    "events": [
        {"kind": "ack", "weight": 1.0, "created_at": "2026-09-05T10:00:00Z", "ref": None},
        {"kind": "duplicate_body", "weight": -2.0, "created_at": "2026-09-05T11:00:00Z", "ref": None},
    ],
    "updated_at": "2026-09-05T11:00:00Z",
}


def test_reputation_prints_the_breakdown(fx, capsys):
    FakeClient.state["reputation_result"] = REPUTATION_FULL
    cli.main(["--config", str(fx), "reputation"])
    out, err = capsys.readouterr()

    assert FakeClient.state["reputation_calls"] == 1
    # the whole answer is the JSON; the summary is for a reader
    assert json.loads(out)["components"]["positive"] == 9.0

    # the score and the three terms that produce it, on one line
    assert "tier T0" in err and "score 7.5 (neutral)" in err
    assert "+9" in err and "3.5" in err and "2 tenure" in err
    # the sybil-resistant gate is worth naming
    assert "2 distinct T1+ counterparties" in err
    assert "4 active days" in err
    assert "2 recent signals" in err
    assert "next: T1 in 7.5 score" in err
    # biggest movers first, whichever direction they moved in
    assert "what moved it: ack +4, reply_cold_open +3, duplicate_body -2" in err


def test_reputation_exits_1_on_a_server_error(fx, capsys):
    FakeClient.state["reputation_result"] = REPUTATION_FULL

    def _boom(self):
        raise LloomError("not_ready", "storage unavailable", 503)

    original = FakeClient.reputation
    FakeClient.reputation = _boom
    try:
        with pytest.raises(SystemExit) as exc:
            cli.main(["--config", str(fx), "reputation"])
    finally:
        FakeClient.reputation = original
    assert exc.value.code == 1
    assert "not_ready" in capsys.readouterr().err
