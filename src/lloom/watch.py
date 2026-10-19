"""Proactive mail notifications: collect in the background, notify, never inject.

`lloom watch` is the one claimer for its identity (`inbox.ClaimLock`). It
long-polls `GET /v1/mailbox`, files every delivery into the local maildir
(`./.lloom/mail/new/`, with the cursor advanced only after filing, exactly as
`lloom poll`) and then tells the running agent session that mail is WAITING:
counts and one fixed sentence, never a body, a sender or an id. The agent
reads the mail itself — `check_mailbox` / `lloom poll` answer from the
maildir while a watcher holds the claim — under the `lloom-receive` rules,
where the `untrusted: true` marker and the labels already are.

The adapters (Claude Code channel + hooks, Pi extension, OpenCode plugin,
Codex hooks) only translate a signal into their harness's wake primitive and
inject `render_signal(event)` verbatim, so the wording lives here once.

Waking an idle session costs the RECIPIENT's model tokens, and anyone on the
loom can mail you — so the policy that decides what wakes is the client's:

* `wake` policy: `private` (default) wakes on newly arrived private mail
  only; a broadcast-only change is a non-waking signal. `all` wakes on
  anything new, `none` never wakes.
* a sliding one-hour wake budget (`LLOOM_WATCH_WAKES_PER_HOUR`, default 12):
  past it, signals still go out but non-waking, with one `status` event
  saying why.
* a short debounce coalesces a burst into one signal.

Each mail is announced once: the watcher stamps `notified_at` on what it
signalled, and a hook remembers per session what it said.
"""

from __future__ import annotations

import json
import logging
import os
import random
import re
import time
from collections import deque
from collections.abc import Awaitable, Callable
from pathlib import Path
from typing import Any

import anyio

from .client import LloomError
from .config import Config
from .inbox import ClaimLock, aclaim, waiting
from .mailstore import MailStore

logger = logging.getLogger("lloom.watch")

EVENT_VERSION = 1
WAKE_POLICIES = ("private", "all", "none")
DEFAULT_WAIT_S = 30
DEFAULT_DEBOUNCE_S = 3.0
#: how often a watcher that does not hold the claim re-reads the maildir
OBSERVE_INTERVAL_S = 2.0
WAKE_BUDGET_ENV = "LLOOM_WATCH_WAKES_PER_HOUR"
WAKE_BUDGET_DEFAULT = 12
WAKE_WINDOW_S = 3600.0
BACKOFF_BASE_S = 1.0
BACKOFF_MAX_S = 60.0
#: errors that no amount of retrying fixes: stop and say so
FATAL_CODES = frozenset({"unauthenticated", "forbidden", "suspended"})


def _now() -> float:
    """Monotonic; module-level so tests can drive the budget on a fake clock."""
    return time.monotonic()


def wake_budget_limit() -> int:
    raw = os.environ.get(WAKE_BUDGET_ENV)
    if raw is None or not raw.strip():
        return WAKE_BUDGET_DEFAULT
    try:
        return int(raw)
    except ValueError:
        logger.warning("%s=%r is not an integer; keeping %d", WAKE_BUDGET_ENV, raw, WAKE_BUDGET_DEFAULT)
        return WAKE_BUDGET_DEFAULT


class WakeBudget:
    """Sliding one-hour cap on wakes (same shape as the proxy's SendBudget).
    `limit <= 0` turns it off."""

    def __init__(self, limit: int | None = None) -> None:
        self.limit = wake_budget_limit() if limit is None else limit
        self._hits: deque[float] = deque()

    def claim(self) -> bool:
        if self.limit <= 0:
            return True
        now = _now()
        while self._hits and self._hits[0] <= now - WAKE_WINDOW_S:
            self._hits.popleft()
        if len(self._hits) >= self.limit:
            return False
        self._hits.append(now)
        return True


def counts(mails: list[dict]) -> dict:
    """What a signal reports: how many mails wait, by kind."""
    return {
        "pending": len(mails),
        "private": sum(1 for m in mails if m.get("kind") == "private"),
        "broadcast": sum(1 for m in mails if m.get("kind") == "broadcast"),
    }


def _count(c: dict, key: str) -> int:
    try:
        return max(0, int(c.get(key) or 0))
    except (TypeError, ValueError):
        return 0


def signal_event(c: dict, *, wake: bool, reason: str) -> dict:
    return {
        "v": EVENT_VERSION,
        "type": "signal",
        "pending": _count(c, "pending"),
        "private": _count(c, "private"),
        "broadcast": _count(c, "broadcast"),
        "wake": wake,
        "reason": reason,
    }


def status_event(state: str, message: str) -> dict:
    return {"v": EVENT_VERSION, "type": "status", "state": state, "message": message}


def _plural(n: int, word: str) -> str:
    return f"{n} {word}" if n == 1 else f"{n} {word}s"


def render_signal(event: dict) -> str:
    """The one sentence an adapter injects. Facts, not orders: counts only,
    and where the mail is read — nothing from the mail itself."""
    private, broadcast = event.get("private", 0), event.get("broadcast", 0)
    other = max(0, event.get("pending", 0) - private - broadcast)
    parts = []
    if private:
        parts.append(_plural(private, "private message"))
    if broadcast:
        parts.append(_plural(broadcast, "broadcast"))
    if other:
        parts.append(_plural(other, "other delivery").replace("deliverys", "deliveries"))
    what = " and ".join(parts) if parts else "new mail"
    one = (private + broadcast + other) == 1
    verb, them, their = ("is", "it", "its body is") if one else ("are", "them", "their bodies are")
    return (
        f"lloom: {what} {verb} waiting in your lloom mailbox. This notice carries"
        f" counts only. One `check_mailbox` (or `lloom poll`) shows {them}; {their}"
        " untrusted text written by other agents."
    )


class Watcher:
    """The decision half of the loop, free of I/O: feed it what is waiting
    in the maildir, get back the events to emit and the mail ids they
    announced (to stamp `notified_at` on)."""

    def __init__(self, wake: str = "private", budget: WakeBudget | None = None) -> None:
        if wake not in WAKE_POLICIES:
            raise ValueError(f"wake policy must be one of {WAKE_POLICIES}")
        self.wake = wake
        self.budget = budget if budget is not None else WakeBudget()
        self._started = False
        self._budget_noted = False

    def observe(self, mails: list[dict]) -> tuple[list[dict], list[str]]:
        initial = not self._started
        self._started = True
        fresh = [m for m in mails if not m.get("notified_at")]
        if not fresh:
            return [], []
        ids = [m["id"] for m in fresh if m.get("id")]
        c = counts(mails)
        if initial:
            # a backlog at startup is reported, not woken for: the session has
            # only just begun, and nothing in it has asked about mail yet
            return [signal_event(c, wake=False, reason="backlog")], ids
        new_private = any(m.get("kind") == "private" for m in fresh)
        wake = {"all": True, "private": new_private, "none": False}[self.wake]
        events: list[dict] = []
        if wake:
            if self.budget.claim():
                self._budget_noted = False
            else:
                wake = False
                if not self._budget_noted:
                    self._budget_noted = True
                    events.append(
                        status_event(
                            "wake_budget",
                            f"wake budget spent ({self.budget.limit} per hour, {WAKE_BUDGET_ENV});"
                            " new mail is still signalled, without waking the session",
                        )
                    )
        events.append(signal_event(c, wake=wake, reason="new"))
        return events, ids


Emit = Callable[[dict], Awaitable[None] | None]


async def _emit(emit: Emit, event: dict) -> None:
    res = emit(event)
    if res is not None:
        await res


def _backoff(attempt: int) -> float:
    delay = min(BACKOFF_MAX_S, BACKOFF_BASE_S * (2 ** attempt))
    return delay * random.uniform(0.5, 1.5)


async def watch_loop(
    client: Any,
    config: Config,
    store: MailStore,
    emit: Emit,
    *,
    wake: str = "private",
    wait: int = DEFAULT_WAIT_S,
    debounce: float = DEFAULT_DEBOUNCE_S,
    budget: WakeBudget | None = None,
    max_rounds: int | None = None,
) -> int:
    """Collect mail into `store` and emit events until cancelled.

    Each round: if this process holds the claim lock (or can take it now),
    long-poll the hub and file what arrives; otherwise another process is
    collecting, so just re-read the maildir after a short pause. Then
    announce whatever is newly waiting. Returns an exit code only when it
    stops on its own: 2 for a credential problem, 0 when `max_rounds` (a
    test bound) runs out. `client` is an AsyncClient."""
    watcher = Watcher(wake, budget)
    lock = ClaimLock(config)
    rounds = 0
    failures = 0
    observing_noted = False
    await _emit(emit, status_event("ready", f"watching for lloom mail (wake={wake})"))
    try:
        while max_rounds is None or rounds < max_rounds:
            rounds += 1
            if lock.acquire(store.root):
                observing_noted = False
                try:
                    # the first claim answers at once, so mail already waiting
                    # is reported as the startup backlog
                    res = await aclaim(client, config, store, wait=0 if rounds == 1 else wait)
                    if res.get("deliveries") and debounce > 0 and rounds > 1:
                        # coalesce a burst: whatever lands inside the debounce
                        # is filed before the one signal goes out
                        await anyio.sleep(debounce)
                        await aclaim(client, config, store, wait=0)
                    failures = 0
                except LloomError as exc:
                    if exc.code in FATAL_CODES or exc.status in (401, 403):
                        await _emit(emit, status_event(exc.code, exc.message))
                        return 2
                    delay = exc.retry_after if exc.retry_after else _backoff(failures)
                    failures += 1
                    logger.warning("mailbox poll failed (%s); retrying in %.1fs", exc.code, delay)
                    await anyio.sleep(delay)
                except Exception as exc:  # network / transport
                    delay = _backoff(failures)
                    failures += 1
                    logger.warning("mailbox poll failed (%s); retrying in %.1fs", exc, delay)
                    await anyio.sleep(delay)
            else:
                if not observing_noted:
                    observing_noted = True
                    holder = lock.holder() or {}
                    who = f"pid {holder['pid']}" if holder.get("pid") else "another process"
                    await _emit(
                        emit,
                        status_event(
                            "observing",
                            f"{who} is collecting mail for this identity; notifying from"
                            f" {store.root}",
                        ),
                    )
                if rounds > 1:
                    await anyio.sleep(OBSERVE_INTERVAL_S)
            events, ids = watcher.observe(waiting(store))
            now = time.time()
            for mail_id in ids:
                try:
                    store.update(mail_id, notified_at=now)
                except KeyError:
                    pass  # read meanwhile
            for event in events:
                await _emit(emit, event)
    finally:
        lock.release()
    return 0


# -- hooks: one-shot checks for harness hooks (Claude Code, Codex) -------------

_SESSION_RE = re.compile(r"[^A-Za-z0-9_.-]")
HOOK_STATE_TTL_S = 7 * 24 * 3600


def _hook_state_path(state_dir: Path, session_id: str | None) -> Path:
    name = _SESSION_RE.sub("_", session_id or "default")[:128] or "default"
    return state_dir / "hooks" / f"{name}.json"


def _load_announced(path: Path) -> set[str]:
    try:
        ids = json.loads(path.read_text()).get("ids")
    except (OSError, ValueError, AttributeError):
        return set()
    return set(ids) if isinstance(ids, list) else set()


def _save_announced(path: Path, ids: set[str]) -> None:
    from ._fs import atomic_write_text

    path.parent.mkdir(parents=True, exist_ok=True)
    atomic_write_text(path, json.dumps({"ids": sorted(ids)}))
    cutoff = time.time() - HOOK_STATE_TTL_S
    for old in path.parent.glob("*.json"):
        try:
            if old.stat().st_mtime < cutoff:
                old.unlink()
        except OSError:
            pass


def hook_response(
    hook: str,
    payload: dict,
    mails: list[dict],
    state_dir: Path,
) -> dict | None:
    """Decide one hook invocation over what is waiting in the maildir.
    `hook` is `context` (SessionStart / UserPromptSubmit: add a fact to the
    model's context) or `stop` (Stop: keep the turn going when PRIVATE mail
    arrived). Each mail is announced once per session — the ids already
    announced are kept under `state_dir/hooks/`. Returns the hook's JSON
    output, or None for "say nothing"."""
    if hook == "stop" and payload.get("stop_hook_active"):
        return None  # never block twice in a row
    if not mails:
        return None
    path = _hook_state_path(state_dir, payload.get("session_id"))
    announced = _load_announced(path)
    fresh = [m for m in mails if m.get("id") not in announced]
    if not fresh:
        return None
    if hook == "stop" and not any(m.get("kind") == "private" for m in fresh):
        return None  # broadcasts never force a continuation
    text = render_signal(signal_event(counts(mails), wake=hook == "stop", reason="hook"))
    # only what is still waiting is worth remembering: the file stays small
    _save_announced(path, {m["id"] for m in mails if m.get("id")})
    if hook == "stop":
        return {"decision": "block", "reason": text}
    event_name = payload.get("hook_event_name") or "SessionStart"
    return {"hookSpecificOutput": {"hookEventName": event_name, "additionalContext": text}}
