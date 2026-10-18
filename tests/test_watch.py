"""Proactive notifications: notify-only, never a claim.

The watcher, the hooks and the Claude Code channel all read the hub's
non-claiming peek (`pending`) and never its claiming `mailbox`; every fake
here records a `mailbox` call so the tests can assert that guarantee.
"""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path
from typing import ClassVar

import anyio
import pytest
from pydantic import TypeAdapter

from lloom import cli, proxy, skills_install, watch
from lloom.client import LloomError
from lloom.watch import WakeBudget, Watcher, hook_response, render_signal, watch_loop

SECRET = {"api_key": "llm_key", "agent_id": "agent:1", "handle": "@me"}


def peek(private=0, broadcast=0, token="t0", newest_at=None):
    return {
        "pending": private + broadcast,
        "private": private,
        "broadcast": broadcast,
        "newest_at": newest_at,
        "token": token,
    }


class ScriptedAsyncClient:
    """`pending()` answers from a script; once it runs dry it blocks, like an
    idle long-poll. `mailbox()` exists only to be caught being called."""

    def __init__(self, script):
        self.script = list(script)
        self.calls: list[dict] = []
        self.mailbox_calls: list[dict] = []

    async def pending(self, since=None, wait=0):
        self.calls.append({"since": since, "wait": wait})
        if not self.script:
            await anyio.sleep(3600)
        item = self.script.pop(0)
        if isinstance(item, Exception):
            raise item
        return item

    async def mailbox(self, **kw):  # pragma: no cover - the assertion is that it never runs
        self.mailbox_calls.append(kw)
        return {"deliveries": []}


def run_loop(client, *, max_peeks, **kw):
    events: list[dict] = []
    kw.setdefault("debounce", 0)

    async def main():
        return await watch_loop(client, events.append, max_peeks=max_peeks, **kw)

    code = anyio.run(main)
    return code, events


@pytest.fixture(autouse=True)
def _no_sleep(monkeypatch):
    real_sleep = anyio.sleep

    async def fast_sleep(seconds):
        await real_sleep(0 if seconds < 3600 else seconds)

    monkeypatch.setattr(watch.anyio, "sleep", fast_sleep)
    monkeypatch.delenv(watch.WAKE_BUDGET_ENV, raising=False)


# -- rendering -------------------------------------------------------------------


def test_render_is_counts_only_and_fact_shaped():
    text = render_signal(watch.signal_event(peek(private=2, broadcast=1), wake=True, reason="new"))
    assert text.startswith("lloom: 2 private messages and 1 broadcast are waiting")
    assert "nothing has been claimed" in text
    assert "untrusted" in text
    single = render_signal(watch.signal_event(peek(private=1), wake=True, reason="new"))
    assert "1 private message is waiting" in single


# -- the decision core -------------------------------------------------------------


def test_backlog_at_start_is_reported_without_waking():
    w = Watcher()
    (ev,) = w.observe(peek(private=2, token="t1"))
    assert ev["type"] == "signal" and ev["wake"] is False and ev["reason"] == "backlog"


def test_empty_start_says_nothing():
    assert Watcher().observe(peek(token="t0")) == []


def test_new_private_mail_wakes_unchanged_token_does_not():
    w = Watcher()
    w.observe(peek(token="t0"))
    (ev,) = w.observe(peek(private=1, token="t1"))
    assert ev["wake"] is True and ev["private"] == 1
    assert w.observe(peek(private=1, token="t1")) == []


def test_broadcast_only_change_signals_without_waking():
    w = Watcher()
    w.observe(peek(token="t0"))
    (ev,) = w.observe(peek(broadcast=1, token="t1"))
    assert ev["wake"] is False


def test_wake_policies():
    w = Watcher(wake="all")
    w.observe(peek(token="t0"))
    assert w.observe(peek(broadcast=1, token="t1"))[0]["wake"] is True
    w = Watcher(wake="none")
    w.observe(peek(token="t0"))
    assert w.observe(peek(private=1, token="t1"))[0]["wake"] is False
    with pytest.raises(ValueError):
        Watcher(wake="sometimes")


def test_wake_budget_turns_wakes_into_signals_with_one_status(monkeypatch):
    w = Watcher(budget=WakeBudget(limit=2))
    w.observe(peek(token="t0"))
    wakes = [w.observe(peek(private=n, token=f"t{n}")) for n in (1, 2, 3, 4)]
    assert [evs[-1]["wake"] for evs in wakes] == [True, True, False, False]
    statuses = [e for evs in wakes for e in evs if e["type"] == "status"]
    assert len(statuses) == 1 and statuses[0]["state"] == "wake_budget"


def test_wake_budget_window_rolls(monkeypatch):
    clock = [0.0]
    monkeypatch.setattr(watch, "_now", lambda: clock[0])
    budget = WakeBudget(limit=1)
    assert budget.claim() is True
    assert budget.claim() is False
    clock[0] = watch.WAKE_WINDOW_S + 1
    assert budget.claim() is True


def test_wake_budget_env(monkeypatch):
    monkeypatch.setenv(watch.WAKE_BUDGET_ENV, "3")
    assert WakeBudget().limit == 3
    monkeypatch.setenv(watch.WAKE_BUDGET_ENV, "lots")
    assert WakeBudget().limit == watch.WAKE_BUDGET_DEFAULT


# -- the loop --------------------------------------------------------------------------


def test_loop_peeks_with_since_and_never_claims():
    client = ScriptedAsyncClient([peek(token="t0"), peek(private=1, token="t1"), peek(private=1, token="t1")])
    code, events = run_loop(client, max_peeks=3, wait=30)
    assert code == 0
    assert client.calls[0] == {"since": None, "wait": 0}
    assert client.calls[1] == {"since": "t0", "wait": 30}
    assert client.calls[2] == {"since": "t1", "wait": 30}
    signals = [e for e in events if e["type"] == "signal"]
    assert len(signals) == 1 and signals[0]["wake"] is True
    assert events[0]["type"] == "status" and events[0]["state"] == "ready"
    assert client.mailbox_calls == []


def test_loop_debounce_supersedes_with_a_fresh_peek():
    client = ScriptedAsyncClient([peek(token="t0"), peek(private=1, token="t1"), peek(private=3, token="t3")])
    _, events = run_loop(client, max_peeks=2, debounce=0.01)
    (signal,) = [e for e in events if e["type"] == "signal"]
    assert signal["private"] == 3
    assert client.calls[-1] == {"since": None, "wait": 0}


def test_loop_stops_on_hub_without_peek():
    client = ScriptedAsyncClient([LloomError("peek_unsupported", "old hub", 404)])
    code, events = run_loop(client, max_peeks=5)
    assert code == 3
    assert events[-1] == {"v": 1, "type": "status", "state": "peek_unsupported", "message": "old hub"}


def test_loop_stops_on_bad_credentials():
    client = ScriptedAsyncClient([LloomError("unauthenticated", "revoked", 401)])
    code, _ = run_loop(client, max_peeks=5)
    assert code == 2


def test_loop_retries_transient_errors():
    client = ScriptedAsyncClient(
        [
            OSError("connection refused"),
            LloomError("quota_exceeded", "slow down", 429, retry_after=0.01),
            peek(token="t0"),
            peek(private=1, token="t1"),
        ]
    )
    code, events = run_loop(client, max_peeks=4)
    assert code == 0
    assert any(e["type"] == "signal" and e["wake"] for e in events)


def test_loop_without_token_paces_instead_of_spinning():
    client = ScriptedAsyncClient([{"pending": 0}, {"pending": 0}, {"pending": 0}])
    run_loop(client, max_peeks=3)
    # no token -> no since -> never a blocking peek that would answer instantly
    assert all(c == {"since": None, "wait": 0} for c in client.calls)


# -- hooks -------------------------------------------------------------------------------


def test_hook_context_announces_once_per_session(tmp_path):
    payload = {"session_id": "s1", "hook_event_name": "UserPromptSubmit"}
    out = hook_response("context", payload, peek(private=1, token="t1"), tmp_path)
    assert out["hookSpecificOutput"]["hookEventName"] == "UserPromptSubmit"
    assert "1 private message is waiting" in out["hookSpecificOutput"]["additionalContext"]
    assert hook_response("context", payload, peek(private=1, token="t1"), tmp_path) is None
    # another session hears about the same mail on its own
    assert hook_response("context", {"session_id": "s2"}, peek(private=1, token="t1"), tmp_path)


def test_hook_stop_blocks_only_on_private_and_never_twice(tmp_path):
    assert hook_response("stop", {"session_id": "s"}, peek(broadcast=2, token="t1"), tmp_path) is None
    assert hook_response("stop", {"session_id": "s", "stop_hook_active": True}, peek(private=1, token="t2"), tmp_path) is None
    out = hook_response("stop", {"session_id": "s"}, peek(private=1, token="t2"), tmp_path)
    assert out["decision"] == "block" and "private message" in out["reason"]
    assert hook_response("stop", {"session_id": "s"}, peek(private=1, token="t2"), tmp_path) is None


def test_hook_nothing_pending_is_silent(tmp_path):
    assert hook_response("context", {}, peek(token="t0"), tmp_path) is None


def test_hook_session_id_cannot_escape_state_dir(tmp_path):
    hook_response("context", {"session_id": "../../etc/x"}, peek(private=1, token="t1"), tmp_path)
    files = list((tmp_path / "hooks").glob("*.json"))
    assert len(files) == 1 and files[0].parent == tmp_path / "hooks"


# -- CLI ------------------------------------------------------------------------------------


class FakeSyncClient:
    state: ClassVar[dict] = {}

    def __init__(self, server_url, api_key=None, timeout=30.0):
        self.state.setdefault("timeouts", []).append(timeout)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return None

    def pending(self, since=None, wait=0):
        self.state.setdefault("pending_calls", []).append({"since": since, "wait": wait})
        err = self.state.get("error")
        if err:
            raise err
        return self.state["peek"]

    def mailbox(self, **kw):  # pragma: no cover
        self.state.setdefault("mailbox_calls", []).append(kw)
        return {"deliveries": []}


@pytest.fixture
def cfx(monkeypatch, tmp_path):
    FakeSyncClient.state = {"peek": peek(private=1, token="t1")}
    monkeypatch.setattr(cli, "Client", FakeSyncClient)
    monkeypatch.delenv("LLOOM_SERVER_URL", raising=False)
    monkeypatch.delenv("LLOOM_CONFIG", raising=False)
    monkeypatch.chdir(tmp_path)
    cfg = tmp_path / "config.json"
    cfg.write_text(json.dumps(SECRET))
    return cfg


def test_cli_watch_once_jsonl(cfx, capsys):
    cli.main(["--config", str(cfx), "watch", "--once", "--format", "jsonl"])
    event = json.loads(capsys.readouterr().out)
    assert event["type"] == "signal" and event["private"] == 1 and event["wake"] is False
    assert FakeSyncClient.state["pending_calls"] == [{"since": None, "wait": 0}]
    assert "mailbox_calls" not in FakeSyncClient.state


def test_cli_watch_once_old_hub_exits_3(cfx, capsys):
    FakeSyncClient.state["error"] = LloomError("peek_unsupported", "old hub", 404)
    with pytest.raises(SystemExit) as exc:
        cli.main(["--config", str(cfx), "watch", "--once"])
    assert exc.value.code == 3


def test_cli_hook_stop_reads_stdin_and_blocks(cfx, capsys, monkeypatch):
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps({"session_id": "abc", "hook_event_name": "Stop"})))
    cli.main(["--config", str(cfx), "hook", "stop"])
    out = json.loads(capsys.readouterr().out)
    assert out["decision"] == "block"
    assert FakeSyncClient.state["timeouts"] == [5.0]


def test_cli_hook_failure_is_a_quiet_noop(cfx, capsys, monkeypatch):
    monkeypatch.setattr(sys, "stdin", io.StringIO("not json"))
    FakeSyncClient.state["error"] = OSError("hub down")
    cli.main(["--config", str(cfx), "hook", "context"])
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "hub down" in captured.err


def test_cli_hook_without_login_is_silent(tmp_path, capsys, monkeypatch):
    monkeypatch.setattr(sys, "stdin", io.StringIO("{}"))
    cfg = tmp_path / "config.json"
    cfg.write_text("{}")
    cli.main(["--config", str(cfg), "hook", "context"])
    assert capsys.readouterr().out == ""


# -- Claude Code channel ----------------------------------------------------------------------


class ChannelAsyncClient:
    script: ClassVar[list] = []

    def __init__(self, server_url, api_key=None, timeout=30.0):
        pass

    async def __aenter__(self):
        return self

    async def __aexit__(self, *exc):
        return None

    async def aclose(self):
        return None

    async def pending(self, since=None, wait=0):
        if not self.script:
            await anyio.sleep(3600)
        return self.script.pop(0)


def test_channel_mode_advertises_capability_and_notifies(monkeypatch):
    from mcp import types
    from mcp.shared.memory import create_client_server_memory_streams
    from mcp.shared.message import SessionMessage

    ChannelAsyncClient.script = [peek(token="t0"), peek(private=1, token="t1"), peek(private=1, token="t1")]
    monkeypatch.setattr(proxy, "AsyncClient", ChannelAsyncClient)
    monkeypatch.setattr(watch, "DEFAULT_DEBOUNCE_S", 0)
    adapter = TypeAdapter(types.JSONRPCMessage)
    seen: dict = {}

    async def main():
        mcp = proxy.build_proxy("http://test", "llm_key", channel=True)
        async with create_client_server_memory_streams() as (client, server), anyio.create_task_group() as tg:
            tg.start_soon(proxy.serve_channel, mcp, "http://test", "llm_key", "private", server)

            async def send(msg):
                await client[1].send(SessionMessage(adapter.validate_python(msg)))

            with anyio.fail_after(10):
                await send(
                    {
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "initialize",
                        "params": {
                            "protocolVersion": "2025-06-18",
                            "capabilities": {},
                            "clientInfo": {"name": "test", "version": "0"},
                        },
                    }
                )
                init = (await client[0].receive()).message.model_dump(by_alias=True, exclude_none=True)
                seen["init"] = init
                await send({"jsonrpc": "2.0", "method": "notifications/initialized"})
                note = (await client[0].receive()).message.model_dump(by_alias=True, exclude_none=True)
                seen["note"] = note
            tg.cancel_scope.cancel()

    anyio.run(main)
    result = seen["init"]["result"]
    assert result["capabilities"]["experimental"] == {"claude/channel": {}}
    assert "<channel" in result["instructions"]
    note = seen["note"]
    assert note["method"] == "notifications/claude/channel"
    assert "1 private message is waiting" in note["params"]["content"]
    meta = note["params"]["meta"]
    assert meta == {"pending": "1", "private": "1", "broadcast": "0"}
    assert all(k.replace("_", "").isalnum() for k in meta)


def test_plain_proxy_has_no_channel_paragraph():
    mcp = proxy.build_proxy("http://test", "llm_key")
    assert "<channel" not in mcp.instructions


def test_channel_notifier_drops_non_waking_events():
    sent: list = []

    class Outbound:
        async def notify(self, method, params):
            sent.append((method, params))

    n = proxy.ChannelNotifier()
    n._outbound = Outbound()

    async def main():
        await n.emit(watch.status_event("ready", "x"))
        await n.emit(watch.signal_event(peek(broadcast=1, token="t"), wake=False, reason="new"))
        await n.emit(watch.signal_event(peek(private=1, token="t"), wake=True, reason="new"))

    anyio.run(main)
    assert [m for m, _ in sent] == [proxy.CHANNEL_METHOD]


# -- installer --------------------------------------------------------------------------------


@pytest.fixture
def push_env(tmp_path, monkeypatch):
    home = tmp_path / "home"
    home.mkdir()
    proj = tmp_path / "proj"
    (proj / ".git").mkdir(parents=True)
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.delenv("CODEX_HOME", raising=False)
    monkeypatch.delenv("XDG_CONFIG_HOME", raising=False)
    monkeypatch.chdir(proj)
    return home, proj


def _tree(root: Path) -> dict:
    return {str(p.relative_to(root)): p.read_bytes() for p in sorted(root.rglob("*")) if p.is_file()}


def test_with_push_project_installs_and_is_idempotent(push_env, capsys):
    _, proj = push_env
    for agent in ("claude-code", "codex", "pi", "opencode"):
        assert skills_install.run_install(agent, "project", False, False, with_push=True) == 0
    assert (proj / ".pi" / "extensions" / "lloom.ts").read_bytes() == skills_install.bundled_integration(
        "pi", "lloom.ts"
    ).read_bytes()
    assert (proj / ".opencode" / "plugins" / "lloom.ts").is_file()
    for settings in (proj / ".claude" / "settings.json", proj / ".codex" / "hooks.json"):
        hooks = json.loads(settings.read_text())["hooks"]
        assert hooks["Stop"][0]["hooks"][0] == {"type": "command", "command": "lloom hook stop"}
        assert hooks["SessionStart"][0]["hooks"][0]["command"] == "lloom hook context"
        assert hooks["UserPromptSubmit"][0]["hooks"][0]["command"] == "lloom hook context"
    before = _tree(proj)
    capsys.readouterr()
    for agent in ("claude-code", "codex", "pi", "opencode"):
        skills_install.run_install(agent, "project", False, False, with_push=True)
    assert _tree(proj) == before
    out = capsys.readouterr().out
    assert "lloom hooks already wired" in out and "CREATE" not in out


def test_with_push_merges_into_existing_settings(push_env):
    _, proj = push_env
    settings = proj / ".claude" / "settings.json"
    settings.parent.mkdir()
    settings.write_text(
        json.dumps(
            {
                "model": "opus",
                "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]},
            }
        )
    )
    skills_install.run_install("claude-code", "project", False, False, with_push=True)
    data = json.loads(settings.read_text())
    assert data["model"] == "opus"
    commands = [h["command"] for g in data["hooks"]["Stop"] for h in g["hooks"]]
    assert commands == ["say done", "lloom hook stop"]


def test_with_push_never_overwrites_a_differing_adapter(push_env, capsys):
    _, proj = push_env
    dest = proj / ".pi" / "extensions" / "lloom.ts"
    dest.parent.mkdir(parents=True)
    dest.write_text("// mine")
    skills_install.run_install("pi", "project", False, False, with_push=True)
    assert dest.read_text() == "// mine"
    assert "MISMATCH" in capsys.readouterr().out


def test_with_push_global_paths(push_env):
    home, _ = push_env
    for agent in ("claude-code", "codex", "pi", "opencode"):
        skills_install.run_install(agent, "global", False, False, with_push=True)
    assert (home / ".pi" / "agent" / "extensions" / "lloom.ts").is_file()
    assert (home / ".config" / "opencode" / "plugins" / "lloom.ts").is_file()
    assert (home / ".claude" / "settings.json").is_file()
    assert (home / ".codex" / "hooks.json").is_file()


def test_with_push_skips_invalid_json(push_env, capsys):
    _, proj = push_env
    dest = proj / ".codex" / "hooks.json"
    dest.parent.mkdir()
    dest.write_text("{nope")
    skills_install.run_install("codex", "project", False, False, with_push=True)
    assert dest.read_text() == "{nope"
    assert "not valid JSON" in capsys.readouterr().out


def test_adapters_ship_in_the_package_and_spawn_the_watcher():
    for agent in ("pi", "opencode"):
        text = skills_install.bundled_integration(agent, "lloom.ts").read_text()
        assert '"watch", "--format", "jsonl"' in text
        # notify-only: an adapter never claims mail itself
        assert "poll" not in text.replace("`lloom poll`", "")
