"""Database-free, file-based mail store for lloom agents (maildir model).

Each mail is a plain-text file with YAML-style frontmatter, stored under
``<root>/{new,read,sent,outbox}/``. The filename is a SHA-256 content hash, so
identical content addresses to the same file and agents can grep the tree with
ordinary shell tools — no database, no index.

Layout::

    <root>/            # default ./.lloom/mail (CWD-scoped)
      new/             # received from the server, not yet read locally
      read/            # read (filed) locally; carries acked_at when acked
      sent/            # accepted by the server (message_id stamped)
      outbox/          # queued to send; dead attempts keep an `error:` line
                       # (embedded sends also carry a <id>.vec.json sidecar
                       # with the exact vector, so retries never re-embed)

Root precedence (when not given explicitly): config key ``mail_dir`` >
env ``LLOOM_MAIL_DIR`` > ``./.lloom/mail``.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import sqlite3
import time
from pathlib import Path
from typing import Any

from ._fs import atomic_write_text, fsync_dir
from .config import Config

FOLDERS = ("new", "read", "sent", "outbox")
KINDS = ("private", "public", "broadcast", "board")
VEC_SUFFIX = ".vec.json"

#: Frontmatter fields holding a SERVER-side id, resolvable like a mail id.
SERVER_ID_KEYS = ("delivery_id", "message_id")
#: The record-table prefixes those ids carry on the wire. Agents paste them
#: stripped as often as whole (`message:0b119aec…` and `0b119aec…` both).
RECORD_PREFIXES = ("delivery:", "message:")

#: `thread_key` and `superseded_by` are deliberately NOT content fields: the
#: server derives both afresh on every poll, so hashing them would file one
#: delivery under two names the second time it is served. `board` IS one: a
#: board post's identity includes which board it belongs to, and an outbox
#: retry must replay the same board under the same idempotency key.
_CONTENT_KEYS = (
    "from",
    "to",
    "kind",
    "body",
    "tags",
    "reply_to",
    "correlation_id",
    "board",
    "delivery_id",
    "message_id",
    "idempotency_key",
    "intent",
    "location",
    "radius_km",
    "expires_at",
)
_META_KEYS = (
    "id",
    "kind",
    "from",
    "to",
    "reply_to",
    "correlation_id",
    "board",
    "thread_key",
    "superseded_by",
    "delivery_id",
    "message_id",
    "idempotency_key",
    "intent",
    "tags",
    "location",
    "radius_km",
    "embed",
    "embed_backend",
    "dead",
    "error",
    "acked_at",
    "created_at",
)


def resolve_mail_root(root: Path | str | None = None, config: Config | None = None) -> Path:
    """Resolve the mail root: explicit root > config `mail_dir` key >
    LLOOM_MAIL_DIR env > ./.lloom/mail (CWD-scoped)."""
    if root is not None:
        return Path(root).expanduser()
    cfg = config if config is not None else Config()
    configured = cfg.get("mail_dir")
    if configured:
        return Path(str(configured)).expanduser()
    env = os.environ.get("LLOOM_MAIL_DIR")
    if env:
        return Path(env).expanduser()
    return Path.cwd() / ".lloom" / "mail"


def strip_record_prefix(value: str) -> str:
    """`delivery:abc` / `message:abc` -> `abc`; anything else unchanged."""
    for prefix in RECORD_PREFIXES:
        if value.startswith(prefix):
            return value[len(prefix) :]
    return value


def _read_text(path: Path) -> str:
    """Read a mail file WITHOUT newline translation.

    ``Path.read_text()`` opens with the default ``newline=None``, which
    applies universal-newline translation (``\\r\\n`` / ``\\r`` -> ``\\n``).
    A body containing ``\\r\\n`` would parse back with ``\\n`` only, so an
    outbox retry after a lost response would replay a DIFFERENT body under
    the same idempotency key — 409 idempotency_conflict, the entry dead
    locally despite successful remote delivery. ``newline=""`` disables all
    translation; every mail file read goes through here.
    """
    with path.open("r", encoding="utf-8", newline="") as f:
        return f.read()


def _atomic_write(path: Path, text: str) -> None:
    """Crash-safe file write: shared tmp+fsync+rename helper (see `_fs`).

    ``os.replace`` is atomic within a folder, so a crash mid-write can never
    leave a truncated file under the final name — a truncated mail file's id
    line would still match its content hash and masquerade as valid.
    """
    atomic_write_text(path, text)


class MailStore:
    def __init__(self, root: Path | str | None = None, config: Config | None = None):
        self._root = resolve_mail_root(root, config).resolve()
        for folder in FOLDERS:
            d = self._root / folder
            d.mkdir(parents=True, exist_ok=True)
            # owner-only: mail bodies/sidecars are private regardless of umask
            d.chmod(0o700)
        self._root.chmod(0o700)

    @property
    def root(self) -> Path:
        return self._root

    # -- public API --------------------------------------------------------

    def receive(self, mail: dict) -> str:
        """File an inbound delivery into new/."""
        return self.store(mail, folder="new")

    def enqueue(self, mail: dict) -> str:
        """File an outbound intent into outbox/ (before any send attempt)."""
        return self.store(mail, folder="outbox")

    # -- embedding sidecar (idempotent retries) ------------------------------

    def _vec_path(self, id_: str) -> Path:
        return self._root / "outbox" / f"{id_}{VEC_SUFFIX}"

    def save_vector(self, id_: str, embedding: list, backend: str) -> None:
        """Persist the EXACT embedding of the original send attempt as a
        sidecar ``outbox/<id>.vec.json`` (keeps the .md grep-friendly).

        A retry must replay the identical vector: a fresh embedding — or
        none at all when the model is unavailable — produces a different
        idempotency digest, so a send the server already committed (lost
        response) would come back as a permanent 409 idempotency_conflict.
        """
        _atomic_write(self._vec_path(id_), json.dumps({"embedding": embedding, "backend": backend}))

    def load_vector(self, id_: str) -> dict | None:
        """Read the sidecar: {"embedding": [...], "backend": ...} or None."""
        path = self._vec_path(id_)
        if not path.exists():
            return None
        try:
            data = json.loads(_read_text(path))
        except ValueError:
            return None
        if not isinstance(data, dict) or not isinstance(data.get("embedding"), list):
            return None
        return data

    def drop_vector(self, id_: str) -> None:
        path = self._vec_path(id_)
        if path.exists():
            path.unlink()

    def store(self, mail: dict, folder: str = "outbox") -> str:
        if folder not in FOLDERS:
            raise ValueError(f"unknown folder: {folder!r} (expected {FOLDERS})")
        self._validate(mail)
        mail = dict(mail)
        mail["id"] = self._hash(mail)
        mail.setdefault("created_at", time.time())
        self._write(mail, folder)
        return mail["id"]

    def get(self, id_: str) -> dict | None:
        for folder in FOLDERS:
            path = self._root / folder / f"{id_}.md"
            if path.exists():
                return self._parse(_read_text(path))
        return None

    def list(self, folder: str) -> list[dict]:
        if folder not in FOLDERS:
            raise ValueError(f"unknown folder: {folder!r} (expected {FOLDERS})")
        out = []
        for path in sorted((self._root / folder).glob("*.md")):
            mail = self._parse(_read_text(path))
            out.append(mail)
        return sorted(out, key=lambda m: m.get("created_at", 0.0))

    def file(self, id_: str) -> dict:
        """Move new/ -> read/ (reading IS filing). Idempotent."""
        return self._move_to_read(id_, acked_at=None)

    def ack(self, id_: str) -> dict:
        """Move new/ -> read/ and stamp acked_at."""
        return self._move_to_read(id_, acked_at=time.time())

    def update(self, id_: str, **fields: Any) -> dict:
        """Rewrite frontmatter fields of a mail in place (whichever folder)."""
        for folder in FOLDERS:
            path = self._root / folder / f"{id_}.md"
            if not path.exists():
                continue
            mail = self._parse(_read_text(path))
            mail.update(fields)
            _atomic_write(path, self._serialize(mail))
            return mail
        raise KeyError(id_)

    def move(self, id_: str, dest: str, **stamp: Any) -> dict:
        """Move a mail to `dest`, stamping frontmatter fields (e.g. message_id).

        The move is a single atomic ``os.replace`` (same filesystem): a crash
        can leave the file in the source folder (stamped) or the destination,
        never in both and never in neither — retries converge instead of
        looping. A pre-existing destination file is overwritten (desired
        idempotency for crash-residue duplicates).
        """
        if dest not in FOLDERS:
            raise ValueError(f"unknown folder: {dest!r} (expected {FOLDERS})")
        # scan every OTHER folder before dest: a leftover duplicate at the
        # destination (crash residue) is overwritten by the atomic rename,
        # not parsed as the authoritative copy
        for folder in [f for f in FOLDERS if f != dest] + [dest]:
            src = self._root / folder / f"{id_}.md"
            if not src.exists():
                continue
            mail = self._parse(_read_text(src))
            if stamp:
                mail.update(stamp)
                _atomic_write(src, self._serialize(mail))
            if folder == dest:
                # the vector sidecar is only meaningful while the entry
                # pends in outbox/: reaching the destination retires it
                self.drop_vector(id_)
                return mail
            os.replace(src, self._root / dest / f"{id_}.md")
            # persist the rename itself: fsync both directories so a host
            # crash cannot lose the entry or resurrect it at the source
            fsync_dir(self._root / folder)
            fsync_dir(self._root / dest)
            # dropped only AFTER the atomic rename: a crash in between
            # leaves an orphan sidecar (harmless litter), never a pending
            # outbox entry whose exact vector was lost
            self.drop_vector(id_)
            return mail
        raise KeyError(id_)

    def delete(self, id_: str, folder: str | None = None) -> None:
        folders = (folder,) if folder else FOLDERS
        for f in folders:
            path = self._root / f / f"{id_}.md"
            if path.exists():
                path.unlink()
                self.drop_vector(id_)
                return

    def find_by_delivery(self, delivery_id: str) -> dict | None:
        """Locate a mail by its server delivery_id (new/ checked first)."""
        for folder in FOLDERS:
            for mail in self.list(folder):
                if mail.get("delivery_id") == delivery_id:
                    return mail
        return None

    def resolve_prefix(self, prefix: str) -> dict:
        """Resolve ANY of the three ids an agent sees to a mail, any folder.

        An agent juggles the local mail id (the content hash in the
        filename), the server's `delivery_id` and its `message_id`, and
        reaches for whichever it last saw printed — every real CLI failure
        in the last scenario run was `lloom mail read` handed one of the
        other two. The mail id is matched by filename glob as before; the
        two server ids are matched by scanning the frontmatter, which
        already carries both, with their `delivery:` / `message:` record
        prefix optional on either side. Every form matches as a full id or
        as a unique prefix — the ids an agent has to hand are truncated
        (`mail ls` prints eight characters) as often as they are whole.

        A mail-id match wins outright: the filename hash is the id this
        store owns. Matches are keyed by mail id, so ONE mail matching on
        both of its server ids — or sitting in two folders (the same
        content re-enqueued while a copy is already in sent/, or crash
        residue) — is one hit and not an ambiguity. Raises KeyError when
        nothing matches, ValueError when the prefix names two mails.
        """
        matches: dict[str, dict] = {}
        for folder in FOLDERS:
            for path in (self._root / folder).glob(f"{prefix}*.md"):
                mail = self._parse(_read_text(path))
                matches.setdefault(mail["id"], mail)
        if not matches:
            matches = self._match_server_ids(prefix)
        if not matches:
            raise KeyError(prefix)
        if len(matches) > 1:
            raise ValueError(f"ambiguous id prefix: {prefix!r} matches {len(matches)} mails")
        return next(iter(matches.values()))

    def _match_server_ids(self, prefix: str) -> dict[str, dict]:
        """Mails whose delivery_id or message_id starts with `prefix`,
        keyed by mail id (folder order: new/ first)."""
        bare = strip_record_prefix(prefix)
        if not bare:  # a naked "message:"/"delivery:" would match everything
            return {}
        out: dict[str, dict] = {}
        for folder in FOLDERS:
            for mail in self.list(folder):
                for key in SERVER_ID_KEYS:
                    value = mail.get(key)
                    if not value:
                        continue
                    value = str(value)
                    if value.startswith(prefix) or strip_record_prefix(value).startswith(bare):
                        out.setdefault(mail["id"], mail)
                        break
        return out

    def thread_key(self, mail: dict) -> str | None:
        """The key that groups this mail's conversation, or None.

        `correlation_id` is the server's own thread key — inherited down a
        reply chain, and self-assigned on a private message that opens one —
        so it is the first thing read. `thread_key` is what a poll derived for
        an inbound delivery, and covers mail whose sender set no key at all.
        A mail with neither is its own thread, named by its message id.
        """
        for key in ("correlation_id", "thread_key"):
            if mail.get(key):
                return str(mail[key])
        return str(mail["message_id"]) if mail.get("message_id") else None

    def thread(self, key: str) -> list[dict]:
        """Every mail in one thread, oldest first.

        A mail belongs when its thread key matches, when it IS the thread root
        (`message_id == key`), or when it answers the root directly
        (`reply_to == key`) — the last two catch mail written before the
        server carried a thread key. The record prefix is optional on either
        side, as everywhere else an id is matched here.

        Ordering is by local file time: the order THIS agent saw the messages,
        which is the order that matters when deciding what to answer. It is
        not the order they were sent — a message can sit on the server between
        one poll and the next.
        """
        bare = strip_record_prefix(key)
        if not bare:
            return []
        out: dict[str, dict] = {}
        for folder in FOLDERS:
            for mail in self.list(folder):
                for field in ("correlation_id", "thread_key", "message_id", "reply_to"):
                    value = mail.get(field)
                    if value and strip_record_prefix(str(value)) == bare:
                        out.setdefault(mail["id"], mail)
                        break
        return sorted(out.values(), key=lambda m: m.get("created_at") or 0.0)

    def search(self, pattern: str) -> list[tuple[str, dict, str]]:
        """Regex-scan every folder's file contents; yield (folder, mail, line)."""
        rx = re.compile(pattern)
        hits: list[tuple[str, dict, str]] = []
        for folder in FOLDERS:
            for path in sorted((self._root / folder).glob("*.md")):
                text = _read_text(path)
                mail = self._parse(text)
                for line in text.split("\n"):
                    if rx.search(line):
                        hits.append((folder, mail, line.strip()))
        return hits

    # -- internals ---------------------------------------------------------

    def _move_to_read(self, id_: str, acked_at: float | None) -> dict:
        read_path = self._root / "read" / f"{id_}.md"
        if read_path.exists():
            mail = self._parse(_read_text(read_path))
            if acked_at is not None and not mail.get("acked_at"):
                return self.update(id_, acked_at=acked_at)
            return mail
        new_path = self._root / "new" / f"{id_}.md"
        if not new_path.exists():
            raise KeyError(id_)
        mail = self._parse(_read_text(new_path))
        if acked_at is not None:
            mail["acked_at"] = acked_at
            _atomic_write(new_path, self._serialize(mail))
        # atomic same-fs rename: the file is in new/ or read/, never both;
        # fsync both dirs so the rename survives a host crash
        os.replace(new_path, read_path)
        fsync_dir(new_path.parent)
        fsync_dir(read_path.parent)
        return mail

    @staticmethod
    def _content_fields(mail: dict) -> dict:
        return {k: mail.get(k) for k in _CONTENT_KEYS}

    def _hash(self, mail: dict) -> str:
        canonical = json.dumps(self._content_fields(mail), sort_keys=True, separators=(",", ":"))
        return hashlib.sha256(canonical.encode("utf-8")).hexdigest()

    @staticmethod
    def _validate(mail: dict) -> None:
        for key in ("from", "kind"):
            if not mail.get(key):
                raise ValueError(f"missing required field: {key!r}")
        # body is required but MAY be empty ("" is a valid wire body): only a
        # missing/None body is invalid input
        if mail.get("body") is None:
            raise ValueError("missing required field: 'body'")
        if mail["kind"] not in KINDS:
            raise ValueError(f"kind must be one of {KINDS}, got {mail['kind']!r}")
        if mail["kind"] == "private" and not str(mail.get("to") or "").strip():
            raise ValueError("private mail requires a non-empty 'to'")
        if mail["kind"] == "board" and not str(mail.get("board") or "").strip():
            raise ValueError("board mail requires a non-empty 'board'")
        # frontmatter scalar fields must be single-line: a \n or \r would
        # corrupt the frontmatter (a newline splits the key's line, and the
        # serializer's newline flattening would replay a DIFFERENT value on
        # the next parse — idempotency_conflict on retry). These are
        # identifiers/handles; newlines are invalid input, fail fast. body
        # is exempt (multi-line by design, stored outside the frontmatter).
        for key in (
            "from", "to", "kind", "reply_to", "correlation_id", "thread_key",
            "superseded_by", "idempotency_key", "intent", "board",
        ):
            v = mail.get(key)
            if isinstance(v, str) and ("\n" in v or "\r" in v):
                raise ValueError(f"{key!r} must be single-line (no newline characters)")
        # scalar identifier fields must round-trip the plain-text frontmatter
        # EXACTLY: the literal strings "null"/"None" parse back as None
        # (sentinel collision) and leading/trailing whitespace is stripped
        # by the parser — either would silently replay a DIFFERENT value on
        # the next read (idempotency digest mismatch → 409 on retry). These
        # values cannot round-trip the plain-text frontmatter: reject them
        # loudly at enqueue instead of corrupting the store silently. body
        # and kind are exempt (body is stored verbatim outside the
        # frontmatter; kind is enum-checked above).
        for key in (
            "from", "to", "reply_to", "correlation_id", "thread_key",
            "superseded_by", "idempotency_key", "intent", "board",
        ):
            v = mail.get(key)
            if isinstance(v, str) and (v in ("null", "None") or v != v.strip()):
                raise ValueError(
                    f"{key!r}={v!r} cannot round-trip the plain-text frontmatter"
                    " (null/None sentinel or leading/trailing whitespace);"
                    " strip the value or pick another one"
                )
        if "tags" in mail and not isinstance(mail["tags"], (list, type(None))):
            raise ValueError("'tags' must be a list")
        loc = mail.get("location")
        if loc is not None and not (isinstance(loc, dict) and "lat" in loc and "lng" in loc):
            raise ValueError("'location' must be a {lat, lng} dict")

    def _write(self, mail: dict, folder: str) -> None:
        path = self._root / folder / f"{mail['id']}.md"
        if path.exists():
            existing = self._parse(_read_text(path))
            if existing.get("id") != mail["id"]:
                raise ValueError(f"content hash collision at {path}")
            return
        _atomic_write(path, self._serialize(mail))

    @staticmethod
    def _fmt_location(loc: dict | None) -> str:
        if not loc:
            return "null"
        return f"{loc.get('lat')},{loc.get('lng')}"

    @staticmethod
    def _parse_tags(value: str) -> list | None:
        # null / missing-line / empty value all mean ABSENT (None) — a
        # present-but-empty `tags: []` must stay distinguishable from an
        # absent tags field (different idempotency digests on the wire)
        value = value.strip()
        if value in ("null", "None", ""):
            return None
        try:
            parsed = json.loads(value)
            if isinstance(parsed, list):
                return parsed
        except ValueError:
            pass
        # legacy format (files written before strict JSON tags): an unquoted
        # bracket list "a, b" — best-effort comma split
        return [t.strip() for t in value.strip("[]").split(",") if t.strip()]

    @staticmethod
    def _parse_location(value: str) -> dict | None:
        if value in ("null", "None", ""):
            return None
        lat, _, lng = value.partition(",")
        return {"lat": float(lat), "lng": float(lng)}

    @classmethod
    def _serialize(cls, mail: dict) -> str:
        def val(key: str) -> str:
            v = mail.get(key)
            if v is None:
                return "null"
            if isinstance(v, bool):
                return "true" if v else "false"
            if isinstance(v, float):
                return repr(v)
            # last-resort newline flattening: identifier fields are rejected
            # at validate time; this only guards unvalidated scalars (error)
            return str(v).replace("\r", " ").replace("\n", " ")

        lines = ["---"]
        lines.append(f"id: {mail.get('id')}")
        for key in (
            "kind",
            "from",
            "to",
            "reply_to",
            "correlation_id",
            "board",
            "thread_key",
            "superseded_by",
            "delivery_id",
            "message_id",
            "idempotency_key",
            "intent",
        ):
            lines.append(f"{key}: {val(key)}")
        # presence-preserving tags: `tags: []` (present, empty) is a
        # DIFFERENT wire payload from a missing tags line (absent) — the
        # server's idempotency digest hashes [] and null differently, so an
        # outbox retry must replay the exact presence of the original send.
        # When tags is absent the line is omitted entirely (a strict JSON
        # array round-trips every string exactly when present).
        tags = mail.get("tags")
        if tags is not None:
            lines.append(f"tags: {json.dumps(tags, ensure_ascii=False)}")
        lines.append(f"location: {cls._fmt_location(mail.get('location'))}")
        lines.append(f"radius_km: {val('radius_km')}")
        lines.append(f"expires_at: {val('expires_at')}")
        lines.append(f"embed: {val('embed')}")
        lines.append(f"embed_backend: {val('embed_backend')}")
        lines.append(f"dead: {val('dead')}")
        lines.append(f"error: {val('error')}")
        lines.append(f"acked_at: {val('acked_at')}")
        lines.append(f"created_at: {val('created_at')}")
        lines.append("---")
        # EXACT body contract: file = frontmatter + "\n" + body + "\n", body
        # VERBATIM (no strip). The body participates in the content hash and
        # in the server-side idempotency digest, so a write->parse round-trip
        # must yield the identical string — an outbox retry that re-parsed a
        # stripped body would send a DIFFERENT request under the same
        # idempotency key (409 idempotency_conflict, entry dead).
        frontmatter = "\n".join(lines) + "\n"
        body = str(mail.get("body") or "")
        return frontmatter + "\n" + body + "\n"

    @classmethod
    def _parse(cls, text: str) -> dict:
        # split("\n") (not splitlines): the body is preserved byte-for-byte
        # — splitlines would also split on \r / \x85 / \u2028 and silently
        # normalize them away on round-trip
        lines = text.split("\n")
        if not lines or lines[0].strip() != "---":
            raise ValueError("not a mail file: missing frontmatter")
        meta: dict[str, Any] = {}
        i = 1
        while i < len(lines) and lines[i].strip() != "---":
            key, _, value = lines[i].partition(":")
            key = key.strip()
            value = value.strip()
            if key == "tags":
                meta["tags"] = cls._parse_tags(value)
            elif key == "location":
                meta["location"] = cls._parse_location(value)
            elif key in ("created_at", "acked_at", "radius_km"):
                meta[key] = None if value in ("null", "None") else float(value)
            elif key in ("embed", "dead"):
                meta[key] = value == "true"
            else:
                # only the literal null sentinels map to None — a written
                # empty string ("reply_to: ") must round-trip as "" (an
                # empty wire value digests differently from an absent one)
                meta[key] = None if value in ("null", "None") else value
            i += 1
        # body = content after the blank line following the closing "---",
        # minus exactly the one terminating newline _serialize appends.
        # Leading/trailing whitespace and embedded newlines survive intact.
        body = "\n".join(lines[i + 1 :]).removeprefix("\n").removesuffix("\n")
        meta["body"] = body
        return meta


def migrate_legacy_store(db_path: Path, store: MailStore, default_from: str = "?") -> int:
    """Drain a legacy SQLite outbox (P6 store.py) into maildir outbox/ files,
    then rename the db to `<name>.migrated` so it is never read again.

    Only queued/failed (non-dead) rows are drained. Returns the drained count.
    """
    if not db_path.exists():
        return 0
    conn = sqlite3.connect(db_path)
    count = 0
    try:
        rows = conn.execute(
            "SELECT payload FROM outbox WHERE status IN ('queued','failed') ORDER BY created_at ASC"
        ).fetchall()
    finally:
        conn.close()
    for (payload_json,) in rows:
        try:
            payload = json.loads(payload_json)
        except (TypeError, ValueError):
            continue
        mail_id = store.enqueue(_mail_from_legacy_payload(payload, default_from))
        emb = payload.get("embedding")
        if emb is not None:
            # a committed-but-unconfirmed legacy send must retry with the
            # EXACT vector it originally carried — re-embedding (or dropping
            # the vector) changes the idempotency digest → permanent 409
            store.save_vector(mail_id, emb, "legacy")
        count += 1
    db_path.rename(db_path.with_name(db_path.name + ".migrated"))
    return count


def _mail_from_legacy_payload(payload: dict, default_from: str) -> dict:
    return {
        "from": default_from,
        "to": payload.get("to"),
        "kind": payload.get("kind", "private"),
        "body": payload.get("body", ""),
        "tags": payload.get("tags"),
        "reply_to": payload.get("reply_to"),
        "correlation_id": payload.get("correlation_id"),
        "idempotency_key": payload.get("idempotency_key"),
        "location": payload.get("location"),
        "radius_km": payload.get("radius_km"),
        "expires_at": payload.get("expires_at"),
        "embed": payload.get("embedding") is not None,
    }
