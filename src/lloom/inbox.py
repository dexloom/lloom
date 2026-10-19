"""The local inbox: claim from the hub, file into the maildir, read it back.

`GET /v1/mailbox` is a claim (every delivery it returns moves
`pending -> read` and starts its lease), so exactly one thing per identity
should be asking. That is the `ClaimLock`: whoever holds it claims, files each
delivery into `./.lloom/mail/new/` and advances the cursor; everyone else
reads the maildir. `lloom watch` holds it for as long as it runs, so with a
watcher up, `lloom poll` and MCP `check_mailbox` answer from `new/` instead of
the hub. Without one they take the lock for a single claim, exactly as
before.

Filing reuses the content-addressed store: the same delivery always lands on
the same file, and one the agent has already read (it sits in `read/`) is
never filed into `new/` a second time, so a re-served delivery is not news.
"""

from __future__ import annotations

import json
import logging
import os
import time
from pathlib import Path
from typing import Any

from ._fs import atomic_write_text
from .cli import EPOCH_CURSOR, _cursor_advances, _encode_client_cursor
from .config import Config
from .mailstore import MailStore

logger = logging.getLogger("lloom.inbox")

try:  # POSIX; elsewhere there is no lock and every caller may claim
    import fcntl
except ImportError:  # pragma: no cover - Windows
    fcntl = None  # type: ignore[assignment]

#: delivery fields the maildir keeps, so a mail read back from new/ carries
#: everything the hub said about it
_DELIVERY_FIELDS = (
    "reply_to",
    "correlation_id",
    "thread_key",
    "superseded_by",
    "board",
    "message_id",
    "sender_tier",
)


class ClaimLock:
    """Advisory `flock` on `<state_dir>/claim.lock`: one claimer per identity.

    The holder writes `{"pid", "mail_root"}` into the file, so a process that
    loses the race can say where the mail is going. flock belongs to the open
    file, so two ClaimLocks in ONE process exclude each other too (the channel
    watcher and `check_mailbox` share a process)."""

    def __init__(self, config: Config) -> None:
        self.path = config.state_dir() / "claim.lock"
        self._fd: int | None = None

    @property
    def held(self) -> bool:
        return self._fd is not None

    def acquire(self, mail_root: Path | None = None) -> bool:
        """Take the lock without blocking. True if this object now holds it."""
        if self._fd is not None:
            return True
        fd = os.open(self.path, os.O_RDWR | os.O_CREAT, 0o600)
        if fcntl is not None:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                os.close(fd)
                return False
        self._fd = fd
        info = json.dumps({"pid": os.getpid(), "mail_root": str(mail_root) if mail_root else None})
        os.ftruncate(fd, 0)
        os.pwrite(fd, info.encode(), 0)
        return True

    def release(self) -> None:
        if self._fd is None:
            return
        fd, self._fd = self._fd, None
        try:
            os.ftruncate(fd, 0)
        except OSError:
            pass
        os.close(fd)  # closing drops the flock

    def holder(self) -> dict | None:
        """Who holds the lock, as its holder wrote it (best effort)."""
        try:
            data = json.loads(self.path.read_text() or "null")
        except (OSError, ValueError):
            return None
        return data if isinstance(data, dict) else None


# -- filing ------------------------------------------------------------------


def file_delivery(store: MailStore, me: str, d: dict) -> str | None:
    """File one hub delivery into new/ (or leave it where it already is).
    Returns the mail id."""
    mail = {
        "from": d["sender"],
        "to": me,
        "kind": d["kind"],
        "body": d["body"],
        "reply_to": d.get("reply_to"),
        "correlation_id": d.get("correlation_id"),
        "thread_key": d.get("thread_key"),
        "superseded_by": d.get("superseded_by"),
        "board": d.get("board"),
        "delivery_id": d["delivery_id"],
        "message_id": d.get("message_id"),
        "labels": d.get("labels") or None,
        "sender_tier": d.get("sender_tier"),
    }
    mail_id = store.receive(mail)
    if d.get("superseded_by"):
        # the server recomputes this on every poll and it is not part of
        # the content hash, so a re-served delivery lands on the file
        # already written: overwrite the annotation rather than keep a
        # stale "nothing newer" from the first time it arrived
        store.update(mail_id, thread_key=d.get("thread_key"), superseded_by=d["superseded_by"])
    return mail_id


def file_all(store: MailStore, me: str, deliveries: Any) -> list[str]:
    """File every well-formed delivery; one the store rejects is logged and
    skipped, never allowed to lose the rest of the page."""
    ids: list[str] = []
    if not isinstance(deliveries, list):
        return ids
    for d in deliveries:
        try:
            mail_id = file_delivery(store, me, d)
        except (KeyError, TypeError, ValueError) as exc:
            logger.warning("delivery not filed (%s): %r", exc, d)
            continue
        if mail_id:
            ids.append(mail_id)
    return ids


# -- the cursor --------------------------------------------------------------


def load_cursor(config: Config, override: str | None = None) -> str:
    if override is not None:
        return override
    path = config.cursor_path()
    cursor = path.read_text().strip() if path.exists() else ""
    # no stored cursor (first-ever poll, or a crash before the cursor was
    # persisted): page from the epoch so un-acked backlog is served
    return cursor or EPOCH_CURSOR


def advance_cursor(config: Config, cursor: str | None, res: dict) -> None:
    """Persist the next cursor — called only AFTER every delivery is filed,
    so a crash mid-filing re-serves the tail (at-least-once) instead of
    losing it. The write is atomic: a torn cursor would pin empty polls."""
    path = config.cursor_path()
    next_cursor = res.get("next_cursor")
    if next_cursor:
        atomic_write_text(path, next_cursor)
    elif res.get("deliveries"):
        # The server emits next_cursor only on a FULL page; ANY partial page
        # would otherwise advance no cursor and every later poll re-serve the
        # same un-acked tail. Synthesize it at the last returned delivery —
        # the (created_at, id) keyset the server encodes — but only when that
        # moves forward: a lease-revert repair sits behind the cursor, and
        # following it would rewind. An EMPTY page keeps the prior cursor.
        last = res["deliveries"][-1]
        synthesized = _encode_client_cursor(last["created_at"], last["delivery_id"])
        if _cursor_advances(cursor, synthesized):
            atomic_write_text(path, synthesized)


def claim(client: Any, config: Config, store: MailStore, *, limit: int = 50, wait: int = 0,
          cursor: str | None = None) -> dict:
    """One claim from the hub: page from the cursor, file, advance."""
    cursor = load_cursor(config, cursor)
    res = client.mailbox(limit=limit, cursor=cursor, wait=wait)
    file_all(store, config.get("handle") or "?", res.get("deliveries", []))
    advance_cursor(config, cursor, res)
    return res


async def aclaim(client: Any, config: Config, store: MailStore, *, limit: int = 50,
                 wait: int = 0) -> dict:
    """`claim` over an AsyncClient."""
    cursor = load_cursor(config)
    res = await client.mailbox(limit=limit, cursor=cursor, wait=wait)
    file_all(store, config.get("handle") or "?", res.get("deliveries", []))
    advance_cursor(config, cursor, res)
    return res


# -- reading back ------------------------------------------------------------


def waiting(store: MailStore) -> list[dict]:
    """Inbound mail no poll has shown yet: in new/ with no `shown_at`,
    oldest first. This is what a notification counts."""
    return [m for m in store.list("new") if not m.get("shown_at")]


def as_delivery(mail: dict) -> dict:
    """A maildir mail in the shape `check_mailbox` / `poll --json` return
    deliveries in, plus the local `mail_id`."""
    out: dict[str, Any] = {
        "delivery_id": mail.get("delivery_id"),
        "sender": mail.get("from"),
        "kind": mail.get("kind"),
        "body": mail.get("body"),
        "labels": mail.get("labels") or [],
        "mail_id": mail.get("id"),
    }
    for key in _DELIVERY_FIELDS:
        out[key] = mail.get(key)
    return out


def take_waiting(store: MailStore, skip_deliveries: Any = ()) -> list[dict]:
    """Hand over what is waiting and stamp it `shown_at`, so the next poll
    does not repeat it. The mail stays in new/ ("not yet read locally")
    until `lloom mail read` or an ack files it. Deliveries in
    `skip_deliveries` (the ones a poll just printed from the hub's own
    answer) are stamped but not returned twice."""
    skip = set(skip_deliveries)
    now = time.time()
    out = []
    for mail in waiting(store):
        if mail.get("delivery_id") not in skip:
            out.append(as_delivery(mail))
        try:
            store.update(mail["id"], shown_at=now)
        except KeyError:
            pass  # read meanwhile
    return out
