"""Proactive notifications: collect in the background, notify, never inject.

The watcher is the one claimer for its identity: it files what the hub
serves into the maildir and announces counts. `lloom poll`, `check_mailbox`
and the hooks then read the maildir instead of claiming over it.
"""

from __future__ import annotations

import asyncio
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
from lloom.config import Config
from lloom.inbox import ClaimLock, take_waiting, waiting
from lloom.mailstore import MailStore
from lloom.watch import WakeBudget, Watcher, hook_response, render_signal, watch_loop

SECRET = {"api_key": "llm_key", "agent_id": "agent:1", "handle": "@me"}

_seq = [0]


def delivery(kind="private", body="hello", sender="@a"):
    _seq[0] += 1
    n = _seq[0]
    return {
        "delivery_id": f"delivery:{n}",
        "message_id": f"message:{n}",
        "sender": sender,
        "kind": kind,
        "body": f"{body} {n}",
        "created_at": f"2026-10-19T00:00:{n:02d}Z",
        "state": "read",
        "labels": [],
    }


def page(*deliveries):
    return {"deliveries": list(deliveries), "next_cursor": None}


def mail(kind="private", id_=None, notified=False):
    _seq[0] += 1
    m = {"id": id_ or f"m{_seq[0]}", "kind": kind}
    if notified:
        m["notified_at"] = 1.0
    return m


@pytest.fixture
def env(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    monkeypatch.delenv("LLOOM_MAIL_DIR", raising=False)
    cfg_path = tmp_path / "config.json"
    cfg_path.write_text(json.dumps(SECRET))
    cfg = Config(cfg_path)
    return cfg, MailStore(config=cfg)


class ScriptedAsyncClient:
    """`mailbox()` answers from a script; once it runs dry it blocks, like an
    idle long-poll."""

    def __init__(self, script):
        self.script = list(script)
        self.calls: list[dict] = []

    async def mailbox(self, limit=50, cursor=None, wait=0):
        self.calls.append({"cursor": cursor, "wait": wait})
        if not self.script:
            await anyio.sleep(3600)
        item = self.script.pop(0)
        if isinstance(item, Exception):
            raise item
        return item


def run_loop(client, cfg, store, *, max_rounds, **kw):
    events: list[dict] = []
    kw.setdefault("debounce", 0)

    async def main():
        return await watch_loop(client, cfg, store, events.append, max_rounds=max_rounds, **kw)

    code = anyio.run(main)
    return code, events


def signals(events):
    return [e for e in events if e["type"] == "signal"]


@pytest.fixture(autouse=True)
def _no_sleep(monkeypatch):
    real_sleep = anyio.sleep

    async def fast_sleep(seconds):
        await real_sleep(0 if seconds < 3600 else seconds)

    monkeypatch.setattr(watch.anyio, "sleep", fast_sleep)
    monkeypatch.delenv(watch.WAKE_BUDGET_ENV, raising=False)


# -- rendering -------------------------------------------------------------------


def test_render_is_counts_only_and_fact_shaped():
    c = {"pending": 3, "private": 2, "broadcast": 1}
    text = render_signal(watch.signal_event(c, wake=True, reason="new"))
    assert text.startswith("lloom: 2 private messages and 1 broadcast are waiting")
    assert "check_mailbox" in text and "untrusted" in text
    single = render_signal(watch.signal_event({"pending": 1, "private": 1}, wake=True, reason="new"))
    assert "1 private message is waiting" in single


# -- the decision core -------------------------------------------------------------


def test_backlog_at_start_is_reported_without_waking():
    w = Watcher()
    (ev,), ids = w.observe([mail(id_="a"), mail(id_="b")])
    assert ev["wake"] is False and ev["reason"] == "backlog" and ev["private"] == 2
    assert ids == ["a", "b"]


def test_empty_start_says_nothing():
    assert Watcher().observe([]) == ([], [])


def test_new_private_mail_wakes_announced_mail_does_not():
    w = Watcher()
    w.observe([])
    (ev,), ids = w.observe([mail(id_="a")])
    assert ev["wake"] is True and ev["private"] == 1 and ids == ["a"]
    assert w.observe([mail(id_="a", notified=True)]) == ([], [])


def test_counts_cover_everything_waiting_not_just_the_new():
    w = Watcher()
    w.observe([])
    (ev,), ids = w.observe([mail(notified=True), mail(id_="new")])
    assert ev["pending"] == 2 and ids == ["new"]


def test_broadcast_only_change_signals_without_waking():
    w = Watcher()
    w.observe([])
    (ev,), _ = w.observe([mail("broadcast")])
    assert ev["wake"] is False


def test_wake_policies():
    w = Watcher(wake="all")
    w.observe([])
    assert w.observe([mail("broadcast")])[0][0]["wake"] is True
    w = Watcher(wake="none")
    w.observe([])
    assert w.observe([mail()])[0][0]["wake"] is False
    with pytest.raises(ValueError):
        Watcher(wake="sometimes")


def test_wake_budget_turns_wakes_into_signals_with_one_status():
    w = Watcher(budget=WakeBudget(limit=2))
    w.observe([])
    wakes = [w.observe([mail()])[0] for _ in range(4)]
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


def test_loop_files_into_the_maildir_then_notifies(env):
    cfg, store = env
    client = ScriptedAsyncClient([page(), page(delivery()), page()])
    code, events = run_loop(client, cfg, store, max_rounds=3, wait=30)
    assert code == 0
    assert events[0]["type"] == "status" and events[0]["state"] == "ready"
    # first claim answers at once; later ones long-poll from the stored cursor
    assert client.calls[0]["wait"] == 0 and client.calls[0]["cursor"] == cli.EPOCH_CURSOR
    assert client.calls[1]["wait"] == 30
    assert client.calls[2]["cursor"] != cli.EPOCH_CURSOR, "cursor advanced after filing"
    (sig,) = signals(events)
    assert sig["wake"] is True and sig["private"] == 1
    (filed,) = store.list("new")
    assert filed["from"] == "@a" and filed["notified_at"]
    assert "hello" not in json.dumps(events), "a signal never carries a body"


def test_loop_startup_backlog_does_not_wake(env):
    cfg, store = env
    client = ScriptedAsyncClient([page(delivery(), delivery())])
    _, events = run_loop(client, cfg, store, max_rounds=1)
    (sig,) = signals(events)
    assert sig["reason"] == "backlog" and sig["wake"] is False and sig["private"] == 2


def test_loop_restart_does_not_reannounce(env):
    cfg, store = env
    run_loop(ScriptedAsyncClient([page(delivery())]), cfg, store, max_rounds=1)
    _, events = run_loop(ScriptedAsyncClient([page()]), cfg, store, max_rounds=1)
    assert signals(events) == []


def test_loop_debounce_coalesces_a_burst(env):
    cfg, store = env
    client = ScriptedAsyncClient([page(), page(delivery()), page(delivery(), delivery())])
    _, events = run_loop(client, cfg, store, max_rounds=2, debounce=0.01)
    (sig,) = signals(events)
    assert sig["private"] == 3
    assert client.calls[-1]["wait"] == 0


def test_loop_stops_on_bad_credentials(env):
    cfg, store = env
    code, events = run_loop(
        ScriptedAsyncClient([LloomError("unauthenticated", "revoked", 401)]), cfg, store, max_rounds=5
    )
    assert code == 2 and events[-1]["state"] == "unauthenticated"


def test_loop_retries_transient_errors(env):
    cfg, store = env
    client = ScriptedAsyncClient(
        [
            OSError("connection refused"),
            LloomError("quota_exceeded", "slow down", 429, retry_after=0.01),
            page(),
            page(delivery()),
        ]
    )
    code, events = run_loop(client, cfg, store, max_rounds=4)
    assert code == 0
    assert any(e["wake"] for e in signals(events))


def test_second_watcher_observes_instead_of_claiming(env):
    cfg, store = env
    holder = ClaimLock(cfg)
    assert holder.acquire(store.root)
    try:
        client = ScriptedAsyncClient([page(delivery())])
        _, events = run_loop(client, cfg, store, max_rounds=2)
        assert client.calls == [], "only the lock holder claims"
        assert any(e.get("state") == "observing" for e in events)
    finally:
        holder.release()
    assert ClaimLock(cfg).acquire(store.root), "released on exit"


# -- reading back while a watcher holds the claim ------------------------------------------


def test_take_waiting_shows_once_and_keeps_mail_in_new(env):
    _, store = env
    from lloom.inbox import file_all

    file_all(store, "@me", [delivery()])
    (d,) = take_waiting(store)
    assert d["sender"] == "@a" and d["mail_id"]
    assert take_waiting(store) == []
    assert len(store.list("new")) == 1 and waiting(store) == []


def test_a_delivery_already_read_is_not_refiled_into_new(env):
    _, store = env
    from lloom.inbox import file_all

    d = delivery()
    (mail_id,) = file_all(store, "@me", [d])
    store.file(mail_id)
    file_all(store, "@me", [d])  # the hub re-serves it after the lease
    assert store.list("new") == []


def test_poll_reads_the_maildir_while_a_watcher_claims(env, capsys, monkeypatch):
    cfg, store = env
    from lloom.inbox import file_all

    file_all(store, "@me", [delivery(body="ping")])
    monkeypatch.setattr(cli, "Client", None)  # any hub call would crash
    lock = ClaimLock(cfg)
    lock.acquire(store.root)
    try:
        cli.main(["--config", str(cfg.path), "poll"])
        out = capsys.readouterr().out
        assert "collects mail for this identity" in out and "ping" in out
        cli.main(["--config", str(cfg.path), "poll", "--json"])
        assert json.loads(capsys.readouterr().out)["deliveries"] == []
    finally:
        lock.release()


def test_check_mailbox_reads_the_maildir_while_a_watcher_claims(env, monkeypatch):
    cfg, store = env
    from lloom.inbox import file_all

    file_all(store, "@me", [delivery(body="ping")])

    class NoHub:
        def __init__(self, *a, **k):
            pass

        async def mailbox(self, **kw):  # pragma: no cover - must not run
            raise AssertionError("claimed over the watcher")

    monkeypatch.setattr(proxy, "AsyncClient", NoHub)
    p = proxy.LloomProxy("http://test", "llm_key", cfg)
    lock = ClaimLock(cfg)
    lock.acquire(store.root)
    try:
        res = json.loads(asyncio.run(p.check_mailbox()))
    finally:
        lock.release()
    (d,) = res["deliveries"]
    assert d["untrusted"] is True and d["body"].startswith("ping")
    assert res["source"] == "maildir"


def test_check_mailbox_files_what_it_claims(env, monkeypatch):
    cfg, store = env
    d = delivery(body="direct")

    class Hub:
        def __init__(self, *a, **k):
            pass

        async def mailbox(self, limit=50, cursor=None, wait=0):
            return page(d)

    monkeypatch.setattr(proxy, "AsyncClient", Hub)
    p = proxy.LloomProxy("http://test", "llm_key", cfg)
    res = json.loads(asyncio.run(p.check_mailbox()))
    assert [x["delivery_id"] for x in res["deliveries"]] == [d["delivery_id"]]
    (filed,) = store.list("new")
    assert filed["delivery_id"] == d["delivery_id"] and filed["shown_at"]


# -- hooks -------------------------------------------------------------------------------


def test_hook_context_announces_once_per_session(tmp_path):
    payload = {"session_id": "s1", "hook_event_name": "UserPromptSubmit"}
    mails = [mail(id_="a")]
    out = hook_response("context", payload, mails, tmp_path)
    assert out["hookSpecificOutput"]["hookEventName"] == "UserPromptSubmit"
    assert "1 private message is waiting" in out["hookSpecificOutput"]["additionalContext"]
    assert hook_response("context", payload, mails, tmp_path) is None
    # another session hears about the same mail on its own
    assert hook_response("context", {"session_id": "s2"}, mails, tmp_path)


def test_hook_stop_blocks_only_on_private_and_never_twice(tmp_path):
    s = {"session_id": "s"}
    assert hook_response("stop", s, [mail("broadcast", id_="b")], tmp_path) is None
    assert hook_response("stop", {**s, "stop_hook_active": True}, [mail(id_="p")], tmp_path) is None
    out = hook_response("stop", s, [mail(id_="p")], tmp_path)
    assert out["decision"] == "block" and "private message" in out["reason"]
    assert hook_response("stop", s, [mail(id_="p")], tmp_path) is None


def test_hook_nothing_waiting_is_silent(tmp_path):
    assert hook_response("context", {}, [], tmp_path) is None


def test_hook_session_id_cannot_escape_state_dir(tmp_path):
    hook_response("context", {"session_id": "../../etc/x"}, [mail()], tmp_path)
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

    def mailbox(self, limit=50, cursor=None, wait=0):
        self.state.setdefault("mailbox_calls", []).append({"cursor": cursor, "wait": wait})
        err = self.state.get("error")
        if err:
            raise err
        return self.state.get("result") or page()


@pytest.fixture
def cfx(monkeypatch, tmp_path):
    FakeSyncClient.state = {"result": page(delivery())}
    monkeypatch.setattr(cli, "Client", FakeSyncClient)
    monkeypatch.delenv("LLOOM_SERVER_URL", raising=False)
    monkeypatch.delenv("LLOOM_CONFIG", raising=False)
    monkeypatch.delenv("LLOOM_MAIL_DIR", raising=False)
    monkeypatch.chdir(tmp_path)
    cfg = tmp_path / "config.json"
    cfg.write_text(json.dumps(SECRET))
    return cfg


def test_cli_watch_once_jsonl(cfx, capsys):
    cli.main(["--config", str(cfx), "watch", "--once", "--format", "jsonl"])
    event = json.loads(capsys.readouterr().out)
    assert event["type"] == "signal" and event["private"] == 1 and event["wake"] is False
    assert FakeSyncClient.state["mailbox_calls"][0]["wait"] == 0
    assert len(list((cfx.parent / ".lloom" / "mail" / "new").glob("*.md"))) == 1


def test_cli_hook_stop_claims_once_and_blocks(cfx, capsys, monkeypatch):
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps({"session_id": "abc", "hook_event_name": "Stop"})))
    cli.main(["--config", str(cfx), "hook", "stop"])
    out = json.loads(capsys.readouterr().out)
    assert out["decision"] == "block"
    assert FakeSyncClient.state["timeouts"] == [5.0]
    # what the hook announced, the next poll shows
    FakeSyncClient.state["result"] = page()
    cli.main(["--config", str(cfx), "poll"])
    assert "hello" in capsys.readouterr().out


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

    async def mailbox(self, limit=50, cursor=None, wait=0):
        if not self.script:
            await anyio.sleep(3600)
        return self.script.pop(0)


def test_channel_mode_advertises_capability_and_notifies(env, monkeypatch):
    from mcp import types
    from mcp.shared.memory import create_client_server_memory_streams
    from mcp.shared.message import SessionMessage

    cfg, _ = env
    ChannelAsyncClient.script = [page(), page(delivery()), page()]
    monkeypatch.setattr(proxy, "AsyncClient", ChannelAsyncClient)
    monkeypatch.setattr(watch, "DEFAULT_DEBOUNCE_S", 0)
    adapter = TypeAdapter(types.JSONRPCMessage)
    seen: dict = {}

    async def main():
        mcp = proxy.build_proxy("http://test", "llm_key", config=cfg, channel=True)
        async with create_client_server_memory_streams() as (client, server), anyio.create_task_group() as tg:
            tg.start_soon(proxy.serve_channel, mcp, "http://test", "llm_key", "private", cfg, server)

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
        await n.emit(watch.signal_event({"pending": 1, "broadcast": 1}, wake=False, reason="new"))
        await n.emit(watch.signal_event({"pending": 1, "private": 1}, wake=True, reason="new"))

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
