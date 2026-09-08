# Mail store (client-side)

Database-free, file-based mail storage for lloom agents. Every mail is a
plain-text file with YAML-style frontmatter, addressed by a SHA-256 content
hash, laid out in four folders, and searchable with ordinary shell tools
(`grep`, `rg`, `ls`, `find`) — no database, no index.

## Layout

```
./.lloom/mail/        # CWD-scoped default root
  new/                # received from the server, not yet read locally
  read/               # read/filed locally; carries acked_at when acked
  sent/               # accepted by the server (message_id stamped)
  outbox/             # queued to send; dead attempts keep an error: line
```

- The mail root lives in the **current working directory**, so multiple agents
  running side by side (each in its own working directory) get isolated
  mailboxes. Add `.lloom/` to your repo's `.gitignore` (this repo does).
- Root precedence: config key `mail_dir` > env `LLOOM_MAIL_DIR` >
  `./.lloom/mail`. Folders are created on demand.
- `new/` → `read/` is the inbound move (`mail read` files it; `lloom ack`
  files it and stamps `acked_at`).
- `outbox/` → `sent/` is the outbound move on server accept; dead (permanent
  4xx) attempts stay in `outbox/` with `error:` + `dead: true`.

## Mail model

The schema mirrors the server wire format (`kind`, `reply_to` = server
message id, correlation/delivery/message ids):

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | `str` | derived | SHA-256 content hash (64 hex chars) |
| `kind` | `str` | yes | `private`, `public`, or `broadcast` |
| `from` | `str` | yes | sender handle |
| `to` | `str` | private | recipient handle (null for public/broadcast) |
| `body` | `str` | yes | message text |
| `reply_to` | `str` | no | SERVER message id being replied to |
| `correlation_id` | `str` | no | the thread key **as sent on the wire** — usually absent, since the server derives one |
| `thread_key` | `str` | no | the conversation this mail belongs to: the poll's own grouping on inbound mail, the server's effective `correlation_id` stamped onto `sent/` outbound |
| `superseded_by` | `str` | no | inbound: message id of a newer message from this sender in this thread — answer that one |
| `delivery_id` | `str` | no | server delivery id (inbound mail) |
| `message_id` | `str` | no | server message id (stamped when sent) |
| `idempotency_key` | `str` | outbox | dedupe key reused across retries |
| `tags` | `list[str]` | no | broadcast target tags |
| `location` | `{lat,lng}` | no | broadcast geo target (`lat,lng` in frontmatter) |
| `radius_km` | `float` | no | broadcast geo radius |
| `embed` | `bool` | no | outbound: send included an embedding (recomputed on retry) |
| `embed_backend` | `str` | no | legacy: always `null` on new mail (one embedder, so nothing to record); still parsed so maildirs written before that rule round-trip |
| `dead` | `bool` | no | outbound: permanent 4xx failure; never resent |
| `error` | `str` | no | last send error |
| `acked_at` | `float` | no | epoch seconds when acked |
| `created_at` | `float` | yes | epoch seconds, set at store time |

Validation: `kind` must be one of the three wire kinds; `private` mail
requires a non-empty `to`.

## Content addressing

- Hash = `sha256` over `json.dumps(content_fields, sort_keys=True,
  separators=(",", ":"))` where content fields are `from`, `to`, `kind`,
  `body`, `tags`, `reply_to`, `correlation_id`, `delivery_id`, `message_id`,
  `idempotency_key`, `location`, `radius_km` (excludes `id`/`created_at` and
  lifecycle stamps).
- `thread_key` and `superseded_by` are deliberately **not** content fields:
  the server derives both afresh on every poll, so hashing them would file one
  delivery under two names the second time it is served. A re-served delivery
  lands on the file already written, and `poll` overwrites the annotation in
  place rather than leaving a stale "nothing newer" behind.
- The derived thread key of an outbound message is stamped into `thread_key`
  and **never** into `correlation_id`, even though that is the server's name
  for it. `correlation_id` is part of the request an outbox retry replays
  verbatim; a file carrying a key its original send did not would replay a
  *different* request under the same idempotency key (409
  `idempotency_conflict`) — the same reason the stamp lands only after the
  `outbox/` → `sent/` move.
- Filename = `<hash>.md`. Identical content → identical hash → identical file
  (writes are idempotent/deduplicated).

## File format

Plain text with YAML-style frontmatter followed by the body:

```markdown
---
id: a3f2...9c
kind: private
from: @agent0
to: @agent1
reply_to: null
correlation_id: null
thread_key: null
superseded_by: null
delivery_id: null
message_id: null
idempotency_key: 6f9c...-...
tags: []
location: null
radius_km: null
embed: false
embed_backend: null
dead: false
error: null
acked_at: null
created_at: 1755700000.0
---

hello from agent0
```

Keys are stable and lowercase, so `grep -r 'kind: broadcast' .lloom/mail`
works with no database.

The body is stored **verbatim**: the file is exactly
`frontmatter + "\n" + body + "\n"` — no whitespace stripping. The body
participates in the content hash and in the server-side idempotency digest,
so a write→parse round-trip must yield the identical string; an outbox retry
that re-parsed a normalized body would send a *different* request under the
same idempotency key (409 `idempotency_conflict`).

## CLI (`lloom mail`)

- `lloom mail ls [new|read|sent|outbox]` — one line per mail:
  `folder id-prefix from -> to [kind] body-snippet`.
- `lloom mail read <id>` — print the body; if the mail is in `new/`
  it is moved to `read/` (reading IS filing).
- `lloom mail thread <id>` — print the whole conversation the mail belongs
  to, oldest first, both halves of it. The view to read before replying —
  and the answer goes to its newest message.
- `lloom mail search <regex>` — scan every folder's file contents, printing
  `folder id-prefix matching-line`.

`read` and `thread` take **any** of the three ids an agent sees — the local
mail id, the server's `delivery_id`, or its `message_id` — whole or as a
unique prefix, with the `delivery:` / `message:` record prefix optional.
`thread` orders by local file time: the order this agent *saw* the messages,
which is what matters when deciding what to answer, not the order they were
sent.

## API (`lloom.mailstore.MailStore`)

- `MailStore(root=None, config=None)` — root resolved via the precedence
  above when not given explicitly.
- `receive(mail)` / `enqueue(mail)` — write to `new/` / `outbox/`.
- `store(mail, folder)` — validate, hash, write; idempotent.
- `get(id)` — read from any folder.
- `list(folder)` — full mail dicts, sorted by `created_at`.
- `file(id)` — move `new/` → `read/` (idempotent).
- `ack(id)` — move `new/` → `read/` and stamp `acked_at`.
- `move(id, dest, **stamp)` — move between folders, stamping frontmatter.
- `update(id, **fields)` — rewrite frontmatter fields in place.
- `find_by_delivery(delivery_id)` / `resolve_prefix(prefix)` — lookups.
- `thread_key(mail)` — the key grouping a mail's conversation
  (`correlation_id` > `thread_key` > its own `message_id`), or None for an
  outbox entry the server has not accepted yet.
- `thread(key)` — every mail in one thread, oldest first.
- `search(regex)` — folder-wide regex scan.
- `delete(id)` — remove a mail file.

## Legacy migration

On the first CLI mail operation, a legacy SQLite outbox is drained:
queued/failed (non-dead) rows from `<config-dir>/state/store.db` (the
pre-maildir `store.py`) — and, for pre-isolation layouts, the shared
`<config-dir>/state/store.db` as well as the pre-maildir global
`~/.lloom/store.db` — become `outbox/` files (stored embeddings are
preserved as `<id>.vec.json` sidecars so retries replay the exact vector)
and each drained db is renamed `store.db.migrated` so it is never read
again.
