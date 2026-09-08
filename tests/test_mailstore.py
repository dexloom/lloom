"""Tests for the maildir mail store (client/src/lloom/mailstore.py)."""

from __future__ import annotations

import json

import pytest

from lloom.mailstore import FOLDERS, MailStore

BASE = {
    "from": "@agent0",
    "to": "@agent1",
    "kind": "private",
    "body": "hello from agent0",
    "tags": ["greeting"],
}


@pytest.fixture
def store(tmp_path) -> MailStore:
    return MailStore(tmp_path / "mail")


def test_layout_created(store):
    for folder in ("new", "read", "sent", "outbox"):
        assert (store.root / folder).is_dir()


def test_store_writes_hash_named_file(store):
    h = store.enqueue(dict(BASE))
    assert len(h) == 64
    assert (store.root / "outbox" / f"{h}.md").exists()
    again = store.enqueue(dict(BASE))
    assert again == h  # content-addressed dedupe


def test_receive_targets_new_folder(store):
    h = store.receive(dict(BASE))
    assert (store.root / "new" / f"{h}.md").exists()
    assert not (store.root / "outbox" / f"{h}.md").exists()


def test_kinds_private_public_broadcast(store):
    assert len(store.enqueue({**BASE, "kind": "private"})) == 64
    assert len(store.enqueue({**BASE, "kind": "public", "to": None})) == 64
    assert len(store.enqueue({**BASE, "kind": "broadcast", "to": None, "tags": ["ops"]})) == 64


def test_validation_errors(store):
    with pytest.raises(ValueError, match="kind"):
        store.enqueue({**BASE, "kind": "direct"})
    with pytest.raises(ValueError, match="to"):
        store.enqueue({**BASE, "kind": "private", "to": " "})
    with pytest.raises(ValueError, match="missing required"):
        store.enqueue({**BASE, "body": None})


def test_reply_to_is_server_message_id(store):
    # reply_to is a SERVER message id, not a local content hash
    mail = {**BASE, "reply_to": "msg:abc123"}
    h = store.enqueue(mail)
    assert store.get(h)["reply_to"] == "msg:abc123"


def test_wire_fields_round_trip(store):
    h = store.enqueue(
        {
            **BASE,
            "reply_to": "msg:orig",
            "correlation_id": "corr-1",
            "delivery_id": "delivery:7",
            "message_id": "msg:7",
            "idempotency_key": "idem-1",
        }
    )
    mail = store.get(h)
    assert mail["correlation_id"] == "corr-1"
    assert mail["delivery_id"] == "delivery:7"
    assert mail["message_id"] == "msg:7"
    assert mail["idempotency_key"] == "idem-1"


def test_file_moves_new_to_read(store):
    h = store.receive(dict(BASE))
    mail = store.file(h)
    assert mail["body"] == "hello from agent0"
    assert (store.root / "read" / f"{h}.md").exists()
    assert not (store.root / "new" / f"{h}.md").exists()
    assert store.file(h)["id"] == h  # idempotent no-op
    with pytest.raises(KeyError):
        store.file("f" * 64)


def test_ack_moves_and_stamps_acked_at(store):
    h = store.receive(dict(BASE, delivery_id="delivery:1"))
    mail = store.ack(h)
    assert (store.root / "read" / f"{h}.md").exists()
    assert not (store.root / "new" / f"{h}.md").exists()
    assert mail["acked_at"] is not None
    assert store.get(h)["acked_at"] == mail["acked_at"]


def test_move_stamps_message_id(store):
    h = store.enqueue(dict(BASE))
    mail = store.move(h, "sent", message_id="msg:42")
    assert (store.root / "sent" / f"{h}.md").exists()
    assert not (store.root / "outbox" / f"{h}.md").exists()
    assert mail["message_id"] == "msg:42"


def test_update_sets_error_and_dead(store):
    h = store.enqueue(dict(BASE))
    store.update(h, error="bad_vector_profile: nope", dead=True)
    mail = store.get(h)
    assert mail["error"] == "bad_vector_profile: nope"
    assert mail["dead"] is True
    assert (store.root / "outbox" / f"{h}.md").exists()


def test_find_by_delivery(store):
    store.receive({**BASE, "delivery_id": "delivery:9"})
    assert store.find_by_delivery("delivery:9") is not None
    assert store.find_by_delivery("delivery:nope") is None


def test_resolve_prefix(store):
    h = store.receive(dict(BASE))
    assert store.resolve_prefix(h[:8])["id"] == h
    with pytest.raises(KeyError):
        store.resolve_prefix("deadbeef")


# -- P1.4: one id space (mail id / delivery id / message id) ------------------


def test_resolve_prefix_accepts_delivery_and_message_ids(store):
    """The three ids an agent juggles all resolve to the same mail, whole or
    truncated, with or without the record prefix — an agent reaches for
    whichever it last saw printed."""
    h = store.receive(
        {**BASE, "delivery_id": "delivery:zzqf86feqm806d54tr2o", "message_id": "message:" + "ab" * 16}
    )
    for token in (
        h,
        h[:8],
        "delivery:zzqf86feqm806d54tr2o",
        "zzqf86feqm806d54tr2o",
        "zzqf86fe",
        "message:" + "ab" * 16,
        "ab" * 16,
        "abababab",
    ):
        assert store.resolve_prefix(token)["id"] == h, token


def test_resolve_prefix_prefers_the_mail_id(store):
    """The filename hash is the id this store owns: a token that is both a
    mail-id prefix and a server-id prefix resolves to the mail it names."""
    named = store.receive({**BASE, "body": "the named one"})
    store.receive({**BASE, "body": "the other one", "delivery_id": f"delivery:{named[:8]}xyz"})
    assert store.resolve_prefix(named[:8])["body"] == "the named one"


def test_resolve_prefix_ambiguous_server_id_raises(store):
    store.receive({**BASE, "body": "one", "message_id": "message:beef0001"})
    store.receive({**BASE, "body": "two", "message_id": "message:beef0002"})
    with pytest.raises(ValueError, match="ambiguous"):
        store.resolve_prefix("beef")


def test_resolve_prefix_one_mail_matching_both_ids_is_not_ambiguous(store):
    h = store.receive({**BASE, "delivery_id": "delivery:dup01", "message_id": "message:dup02"})
    assert store.resolve_prefix("dup0")["id"] == h


def test_resolve_prefix_bare_record_prefix_matches_nothing(store):
    """A naked `message:` would otherwise prefix-match every mail."""
    store.receive({**BASE, "message_id": "message:abc"})
    with pytest.raises(KeyError):
        store.resolve_prefix("message:")


def test_location_round_trip(store):
    h = store.enqueue(
        {
            **BASE,
            "kind": "broadcast",
            "to": None,
            "location": {"lat": 41.5, "lng": 2.1},
            "radius_km": 5.0,
        }
    )
    mail = store.get(h)
    assert mail["location"] == {"lat": 41.5, "lng": 2.1}
    assert mail["radius_km"] == 5.0


def test_distinct_idempotency_keys_distinct_files(store):
    h1 = store.enqueue({**BASE, "idempotency_key": "a"})
    h2 = store.enqueue({**BASE, "idempotency_key": "b"})
    assert h1 != h2


def test_greppable_plain_text(store):
    h = store.enqueue(dict(BASE))
    text = (store.root / "outbox" / f"{h}.md").read_text(encoding="utf-8")
    assert "from: @agent0" in text
    assert "kind: private" in text
    assert "hello from agent0" in text


def test_search_scans_all_folders(store):
    store.receive({**BASE, "body": "needle in new"})
    read_id = store.receive({**BASE, "body": "needle in new2"})
    store.file(read_id)
    hits = store.search("needle")
    folders = {folder for folder, _, _ in hits}
    assert folders == {"new", "read"}


def test_delete_removes_file(store):
    h = store.enqueue(dict(BASE))
    store.delete(h)
    assert store.get(h) is None


def test_atomic_write_no_partial_file_when_rename_fails(store, monkeypatch):
    """A crash between tmp-write and rename never leaves a partial file under
    the final name (a truncated file's id line would still hash-match)."""
    import os

    real_replace = os.replace
    calls = {"n": 0}

    def flaky_replace(src, dst):
        calls["n"] += 1
        if calls["n"] == 1:
            raise OSError("simulated crash before rename")
        return real_replace(src, dst)

    monkeypatch.setattr(os, "replace", flaky_replace)
    with pytest.raises(OSError):
        store.enqueue(dict(BASE))
    assert not list((store.root / "outbox").glob("*.md"))  # no partial final file

    # the retry succeeds and leaves no .tmp files behind
    h = store.enqueue(dict(BASE))
    assert (store.root / "outbox" / f"{h}.md").exists()
    assert not list((store.root / "outbox").glob(".*.tmp"))


def test_body_round_trips_verbatim(store):
    """F7: the body participates in the content hash and the server-side
    idempotency digest, so write->parse must yield the EXACT string —
    leading/trailing whitespace and embedded newlines survive."""
    bodies = [
        "  padded  ",
        "\nleading blank line",
        "trailing newline\n",
        "line one\n\nline three",
        "  both  \n  ends  \n",
        "no whitespace at all",
    ]
    for b in bodies:
        h = store.enqueue({**BASE, "body": b})
        assert store.get(h)["body"] == b, repr(b)
        # the raw file preserves the body byte-for-byte after the blank line
        raw = (store.root / "outbox" / f"{h}.md").read_text(encoding="utf-8")
        assert raw.endswith("\n" + b + "\n"), repr(raw)
        # re-filing (move/update re-serialize a parsed mail) keeps it stable
        store.update(h, error="transient")
        assert store.get(h)["body"] == b


def test_crlf_and_cr_bodies_round_trip_byte_identical(store):
    """F1 (review): Path.read_text() applies universal-newline translation
    (CRLF/CR -> LF), so a body with \\r\\n parsed back as \\n only — an
    outbox retry would replay a DIFFERENT body under the same idempotency
    key. Reads must be translation-free (newline="")."""
    from lloom.cli import _payload_from_mail

    bodies = [
        "crlf line\r\nsecond crlf",
        "bare\rcarriage return",
        "mixed\r\nline\rend\r\n",
        "trailing\r\n",
    ]
    for body in bodies:
        h = store.enqueue({**BASE, "body": body, "idempotency_key": "crlf-key"})
        raw = (store.root / "outbox" / f"{h}.md").read_bytes()
        # write -> read is byte-identical: the exact body survives parsing
        read_back = store.get(h)
        assert read_back["body"] == body, repr(body)
        # the on-disk file carries the body byte-for-byte (exact file contract)
        assert raw.decode("utf-8").endswith("\n" + body + "\n"), repr(body)
        # re-serializing the parsed mail preserves the body bytes and is
        # stable under a second parse
        reserialized = MailStore._serialize(read_back)
        assert reserialized.endswith("\n" + body + "\n"), repr(body)
        assert MailStore._parse(reserialized)["body"] == body
        # the retry path replays the identical body under the same key
        payload = _payload_from_mail(read_back)
        assert payload["body"] == body
        assert payload["idempotency_key"] == "crlf-key"
        # every re-reading path (update stamp, cross-folder move) is stable
        store.update(h, error="transient")
        assert store.get(h)["body"] == body
        store.move(h, "sent", message_id="message:9")
        assert store.get(h)["body"] == body


def test_body_empty_round_trips(store):
    """F2 (review): body is required but MAY be empty — the wire accepts ""
    and a polled empty-body delivery must be fileable. File contract keeps
    the body verbatim at the serialize AND store level."""
    text = MailStore._serialize(
        {"id": "x" * 64, "kind": "private", "from": "@a", "to": "@b", "body": ""}
    )
    assert text.endswith("---\n\n\n")
    assert MailStore._parse(text)["body"] == ""
    h = store.enqueue({**BASE, "body": ""})
    assert store.get(h)["body"] == ""
    assert (store.root / "outbox" / f"{h}.md").exists()
    # None is still invalid input (required key)
    with pytest.raises(ValueError, match="missing required"):
        store.enqueue({**BASE, "body": None})


def test_tags_round_trip_exact_with_commas_brackets_quotes(store):
    """F3 (review): frontmatter tags must round-trip EXACTLY. The old
    bracket-split format was lossy — a tag containing a comma parsed back
    SPLIT, so an outbox retry replayed different tags under the same
    idempotency key. Strict JSON arrays round-trip every string."""
    tags = [
        "ops,ci",
        "a]b",
        'he said "hi"',
        "[not-a-list]",
        "x: y",
        "a\\b",
        "",
        "plain",
    ]
    h = store.enqueue({**BASE, "tags": tags})
    mail = store.get(h)
    assert mail["tags"] == tags
    # re-serialization of the parsed mail is stable (retry path)
    store.update(h, error="transient")
    assert store.get(h)["tags"] == tags
    # re-enqueueing the parsed mail addresses the same file (same hash)
    assert store.enqueue({**mail, "tags": tags}) == h


def test_scalar_fields_with_newlines_rejected_at_enqueue(store):
    """F3 (review): identifier/handle scalars live on single frontmatter
    lines — a newline would corrupt the file (or flatten to a space and
    replay a DIFFERENT value on retry). Fail fast at write time instead."""
    for key in ("from", "to", "reply_to", "correlation_id", "idempotency_key"):
        with pytest.raises(ValueError, match=key):
            store.enqueue({**BASE, key: "line1\nline2"})
        with pytest.raises(ValueError, match=key):
            store.enqueue({**BASE, key: "carriage\rreturn"})
    # kind with a newline is rejected by the kind-enum check (also ValueError)
    with pytest.raises(ValueError, match="kind"):
        store.enqueue({**BASE, "kind": "private\nX"})
    # body is exempt: multi-line by design, stored outside the frontmatter
    assert len(store.enqueue({**BASE, "body": "line1\nline2\r\nline3"})) == 64


def test_legacy_tags_frontmatter_still_parses(store):
    """F3 backward compat: files written by the previous serializer carry
    unquoted `tags: [a, b]` — not valid JSON, so parsing falls back to the
    legacy bracket-split and old files keep reading."""
    legacy = (
        "---\n"
        f"id: {'a' * 64}\n"
        "kind: private\n"
        "from: @old\n"
        "to: @new\n"
        "reply_to: null\n"
        "correlation_id: null\n"
        "delivery_id: null\n"
        "message_id: null\n"
        "idempotency_key: null\n"
        "tags: [ops, ci]\n"
        "location: null\n"
        "radius_km: null\n"
        "embed: false\n"
        "embed_backend: null\n"
        "dead: false\n"
        "error: null\n"
        "acked_at: null\n"
        "created_at: 1755700000.0\n"
        "---\n"
        "\n"
        "written by the old serializer\n"
    )
    (store.root / "new" / f"{'a' * 64}.md").write_text(legacy, encoding="utf-8", newline="")
    mail = store.get("a" * 64)
    assert mail["tags"] == ["ops", "ci"]
    assert mail["body"] == "written by the old serializer"
    # single-tag legacy list (no comma) also parses
    single = legacy.replace("tags: [ops, ci]", "tags: [ops]")
    (store.root / "new" / f"{'a' * 64}.md").write_text(single, encoding="utf-8", newline="")
    assert store.get("a" * 64)["tags"] == ["ops"]
    # empty legacy list parses empty (via JSON: "[]" is valid JSON)
    empty = legacy.replace("tags: [ops, ci]", "tags: []")
    (store.root / "new" / f"{'a' * 64}.md").write_text(empty, encoding="utf-8", newline="")
    assert store.get("a" * 64)["tags"] == []


def test_outbox_retry_payload_body_identical(store):
    """F7 no-strip regression in the retry path: an outbox entry read back
    (what `lloom retry` / the MCP proxy re-send) carries the IDENTICAL body
    that was enqueued, so the re-send produces the same idempotency digest
    instead of a 409 idempotency_conflict."""
    from lloom.cli import _payload_from_mail

    body = "  exact\nretry body  \n"
    h = store.enqueue({**BASE, "body": body, "idempotency_key": "retry-key-1"})
    read_back = store.get(h)
    assert read_back["body"] == body
    payload = _payload_from_mail(read_back)
    assert payload["body"] == body
    assert payload["idempotency_key"] == "retry-key-1"


def test_cross_folder_move_crash_before_rename_leaves_single_file(store, monkeypatch):
    """F8: a crash right before the cross-folder rename leaves the file ONLY
    in the source folder (stamped) — never duplicated, never lost; the retry
    converges instead of looping."""
    import os
    from pathlib import Path

    real_replace = os.replace

    def crash_before_move(src, dst):
        if Path(src).parent != Path(dst).parent:
            raise OSError("simulated crash before cross-folder rename")
        return real_replace(src, dst)

    monkeypatch.setattr(os, "replace", crash_before_move)
    h = store.enqueue(dict(BASE))
    with pytest.raises(OSError):
        store.move(h, "sent", message_id="message:1")
    # only in source; the stamp DID land (in-place rewrite before the rename)
    assert (store.root / "outbox" / f"{h}.md").exists()
    assert not (store.root / "sent" / f"{h}.md").exists()

    monkeypatch.setattr(os, "replace", real_replace)
    mail = store.move(h, "sent")
    assert (store.root / "sent" / f"{h}.md").exists()
    assert not (store.root / "outbox" / f"{h}.md").exists()
    assert mail["message_id"] == "message:1"  # stamp survived the crash


def test_move_overwrites_preexisting_destination(store):
    """F8 idempotency: a leftover duplicate at the destination is overwritten
    (single file), not duplicated."""
    h = store.enqueue(dict(BASE))
    (store.root / "sent" / f"{h}.md").write_text("stale duplicate", encoding="utf-8")
    store.move(h, "sent", message_id="message:2")
    assert not (store.root / "outbox" / f"{h}.md").exists()
    assert (store.root / "sent" / f"{h}.md").read_text(encoding="utf-8") != "stale duplicate"
    assert store.get(h)["message_id"] == "message:2"


def test_file_and_ack_use_atomic_rename(store, monkeypatch):
    """F8: file()/ack() move new/ -> read/ via a single atomic os.replace —
    no write-dest-then-unlink duplication window."""
    import os

    real_replace = os.replace
    cross: list[tuple[str, str]] = []

    def spy(src, dst):
        if "/new/" in str(src) and "/read/" in str(dst):
            cross.append((str(src), str(dst)))
        return real_replace(src, dst)

    monkeypatch.setattr(os, "replace", spy)
    h1 = store.receive(dict(BASE, delivery_id="delivery:11", body="file me"))
    store.file(h1)
    h2 = store.receive(dict(BASE, delivery_id="delivery:12", body="ack me"))
    store.ack(h2)

    assert len(cross) == 2  # both moves were one-shot renames
    assert not any((store.root / "new" / f"{h}.md").exists() for h in (h1, h2))
    assert all((store.root / "read" / f"{h}.md").exists() for h in (h1, h2))


def test_all_write_paths_leave_no_tmp_files(store):
    """Every content-writing path (store, file, ack, update, move) goes
    through the atomic tmp+rename helper."""
    h = store.receive(dict(BASE, delivery_id="delivery:1"))
    store.file(h)  # new -> read
    store.update(h, error="transient")  # frontmatter rewrite in place
    store.ack(h)  # acked_at stamp
    store.move(h, "sent", message_id="message:1")  # cross-folder move
    for folder in ("new", "read", "sent", "outbox"):
        assert not list((store.root / folder).glob(".*.tmp")), folder
    mail = store.get(h)
    assert mail["message_id"] == "message:1"
    assert mail["acked_at"] is not None


def test_list_sorted_metadata(store):
    store.enqueue({**BASE, "body": "one"})
    store.enqueue({**BASE, "body": "two"})
    mails = store.list("outbox")
    assert [m["body"] for m in mails] == ["one", "two"]


def test_root_precedence(tmp_path, monkeypatch):
    # isolate from the developer's real config (config key mail_dir ranks above env)
    monkeypatch.setenv("LLOOM_CONFIG", str(tmp_path / "isolate.json"))
    # explicit root wins
    explicit = tmp_path / "explicit"
    assert MailStore(explicit).root == explicit.resolve()
    # then env
    env_root = tmp_path / "envmail"
    monkeypatch.setenv("LLOOM_MAIL_DIR", str(env_root))
    assert MailStore().root == env_root.resolve()
    # config mail_dir key beats env
    cfg_path = tmp_path / "isolate.json"
    cfg_path.write_text(json.dumps({"mail_dir": (tmp_path / "cfgmail").as_posix()}))
    assert MailStore().root == (tmp_path / "cfgmail").resolve()
    cfg_path.write_text("{}")
    # then CWD default
    monkeypatch.delenv("LLOOM_MAIL_DIR")
    monkeypatch.chdir(tmp_path)
    assert MailStore().root == (tmp_path / ".lloom" / "mail").resolve()


# -- review F1: embedding sidecar -------------------------------------------------


def test_vec_sidecar_save_load_drop_and_retirement(store):
    """F1 (review): the enqueue-time embedding lives in a sidecar
    `<id>.vec.json` next to the outbox mail (the .md itself stays
    grep-friendly plain text); move-to-sent and delete both retire it."""
    h = store.enqueue(dict(BASE))
    assert store.load_vector(h) is None  # nothing saved yet

    store.save_vector(h, [0.1, 0.2], "model")
    sidecar = store.root / "outbox" / f"{h}.vec.json"
    assert sidecar.exists()
    assert store.load_vector(h) == {"embedding": [0.1, 0.2], "backend": "model"}
    # the mail file stays plain text (no floats injected)
    assert "0.1" not in (store.root / "outbox" / f"{h}.md").read_text(encoding="utf-8")

    store.drop_vector(h)
    assert store.load_vector(h) is None

    # a corrupt sidecar reads as absent (retry falls back to re-embedding)
    store.save_vector(h, [0.3], "hash")
    sidecar.write_text("{not json", encoding="utf-8")
    assert store.load_vector(h) is None

    # a successful move (server accept -> sent/) retires the sidecar
    store.save_vector(h, [0.4], "model")
    store.move(h, "sent", message_id="message:1")
    assert (store.root / "sent" / f"{h}.md").exists()
    assert not (store.root / "outbox" / f"{h}.vec.json").exists()
    assert not (store.root / "sent" / f"{h}.vec.json").exists()

    # delete() removes any leftover sidecar with the mail
    h2 = store.enqueue({**BASE, "body": "second"})
    store.save_vector(h2, [0.5], "model")
    store.delete(h2)
    assert not (store.root / "outbox" / f"{h2}.vec.json").exists()


# -- review F2: tags presence preservation -----------------------------------------


def test_tags_presence_preserved_empty_vs_absent(store):
    """F2 (review): `tags: []` (present, empty) and a missing tags line
    (absent) hash differently in the server's idempotency digest; the
    frontmatter must round-trip which one the original send carried. The
    old serializer normalized absent -> `tags: []`, so a retry replayed a
    different payload under the same idempotency key."""
    empty = store.enqueue({"from": "@a", "to": None, "kind": "broadcast", "body": "b", "tags": []})
    text = (store.root / "outbox" / f"{empty}.md").read_text(encoding="utf-8")
    assert "tags: []" in text
    assert store.get(empty)["tags"] == []
    # update/move re-serialize a parsed mail without collapsing the line
    store.update(empty, error="transient")
    assert store.get(empty)["tags"] == []

    absent = store.enqueue({"from": "@a", "to": None, "kind": "broadcast", "body": "b"})
    text = (store.root / "outbox" / f"{absent}.md").read_text(encoding="utf-8")
    assert "tags:" not in text
    assert "tags" not in store.get(absent)
    store.update(absent, error="transient")
    assert "tags" not in store.get(absent)

    # a `tags: null` line (hand-edited file) also reads as absent
    h = store.enqueue({"from": "@a", "to": None, "kind": "broadcast", "body": "c"})
    path = store.root / "outbox" / f"{h}.md"
    path.write_text(
        path.read_text(encoding="utf-8").replace("kind:", "tags: null\nkind:"),
        encoding="utf-8",
        newline="",
    )
    assert store.get(h)["tags"] is None


def test_mail_root_and_files_owner_only(tmp_path):
    """Review round 10: mail bodies/sidecars stay private regardless of the
    process umask (folders 0700, files 0600)."""
    import stat

    store = MailStore(tmp_path / "mail")
    assert stat.S_IMODE(store.root.stat().st_mode) == 0o700
    for folder in FOLDERS:
        assert stat.S_IMODE((store.root / folder).stat().st_mode) == 0o700
    mid = store.enqueue(
        {"from": "@a", "to": "@b", "kind": "private", "body": "secret", "idempotency_key": "k"}
    )
    f = store.root / "outbox" / f"{mid}.md"
    assert stat.S_IMODE(f.stat().st_mode) == 0o600
    store.save_vector(mid, [0.1], "model")
    sidecar = store.root / "outbox" / f"{mid}.vec.json"
    assert stat.S_IMODE(sidecar.stat().st_mode) == 0o600


def test_empty_string_scalars_round_trip_exactly(tmp_path):
    """Round-12 review: an empty-string reply_to/correlation_id is a valid
    wire value that digests differently from an absent one — it must not be
    normalized to None when the outbox file is reread for a retry."""
    store = MailStore(tmp_path / "mail")
    mail = {
        "from": "@a",
        "to": "@b",
        "kind": "private",
        "body": "x",
        "idempotency_key": "k-empty-scalar",
        "reply_to": "",
        "correlation_id": "",
    }
    mid = store.enqueue(dict(mail))
    back = store.get(mid)
    assert back["reply_to"] == ""
    assert back["correlation_id"] == ""

    absent = dict(mail, idempotency_key="k-null-scalar")
    absent.pop("reply_to")
    absent.pop("correlation_id")
    mid2 = store.enqueue(absent)
    back2 = store.get(mid2)
    assert back2["reply_to"] is None
    assert back2["correlation_id"] is None


def test_scalar_sentinels_and_padding_rejected_at_enqueue(store):
    """F4: the literal null sentinels and padded values CANNOT round-trip
    the plain-text frontmatter (the parser maps "null"/"None" back to None
    and strips leading/trailing whitespace), so enqueue fails loudly naming
    the field instead of silently replaying a different value on retry."""
    bad_values = ["null", "None", " pad-left", "pad-right ", " pad-both "]
    for field in ("from", "to", "reply_to", "correlation_id", "idempotency_key"):
        for v in bad_values:
            with pytest.raises(ValueError, match=f"'{field}'"):
                store.enqueue({**BASE, field: v})

    # normal values (including case variants, internal whitespace and the
    # empty string) are unaffected and still round-trip exactly
    ok = {
        "from": "@agent0",
        "to": "@agent1",
        "reply_to": "NULL",
        "correlation_id": "null-x",
        "idempotency_key": "None-like",
    }
    h = store.enqueue({**BASE, **ok})
    back = store.get(h)
    for k, v in ok.items():
        assert back[k] == v
    h2 = store.enqueue({**BASE, "reply_to": "", "correlation_id": ""})
    back2 = store.get(h2)
    assert back2["reply_to"] == ""
    assert back2["correlation_id"] == ""


def test_resolve_prefix_one_mail_in_two_folders_is_not_ambiguous(store):
    """A dedupe-keyed re-send enqueues content that is already in sent/, so
    the same mail id can be under two folders for the length of one send."""
    h = store.enqueue(dict(BASE))
    store.move(h, "sent", message_id="message:1")
    assert store.enqueue(dict(BASE)) == h
    assert (store.root / "sent" / f"{h}.md").exists()
    assert (store.root / "outbox" / f"{h}.md").exists()
    assert store.resolve_prefix(h[:8])["id"] == h


# -- P3.1 / P3.2: thread key and the supersession annotation -------------------


def test_thread_annotation_round_trips_the_frontmatter(store):
    """`thread_key` / `superseded_by` are ordinary frontmatter, greppable
    like every other field."""
    h = store.receive(
        {
            **BASE,
            "delivery_id": "delivery:d1",
            "message_id": "message:m1",
            "thread_key": "message:root",
            "superseded_by": "message:m2",
        }
    )
    back = store.get(h)
    assert back["thread_key"] == "message:root"
    assert back["superseded_by"] == "message:m2"
    text = (store.root / "new" / f"{h}.md").read_text()
    assert "thread_key: message:root" in text
    assert "superseded_by: message:m2" in text


def test_thread_annotation_is_not_part_of_the_content_hash(store):
    """The server derives both afresh on every poll, so hashing them would
    file ONE delivery under two names the second time it is served."""
    mail = {**BASE, "delivery_id": "delivery:d1", "message_id": "message:m1"}
    first = store.receive({**mail, "superseded_by": None})
    second = store.receive({**mail, "superseded_by": "message:m2"})
    assert first == second
    # ...and refreshing the annotation in place keeps the same file
    store.update(first, superseded_by="message:m2")
    assert store.get(first)["superseded_by"] == "message:m2"
    assert len(list((store.root / "new").glob("*.md"))) == 1


def test_thread_key_prefers_the_correlation_id(store):
    mail = {**BASE, "message_id": "message:m1", "correlation_id": "message:root",
            "thread_key": "message:other"}
    assert store.thread_key(mail) == "message:root"
    assert store.thread_key({**BASE, "message_id": "message:m1",
                             "thread_key": "message:root"}) == "message:root"
    # neither: the message is its own thread
    assert store.thread_key({**BASE, "message_id": "message:m1"}) == "message:m1"
    # an outbox entry the server has not accepted yet has no id at all
    assert store.thread_key(dict(BASE)) is None


def test_thread_collects_both_halves_of_a_conversation(store):
    """The view an agent needs before answering: what they said and what I
    said, in the order I saw them."""
    root = "message:root"
    inbound = store.receive({
        **BASE, "from": "@marc", "to": "@carla", "body": "shall we say four?",
        "delivery_id": "delivery:d1", "message_id": root, "correlation_id": root,
        "created_at": 100.0,
    })
    mine = store.enqueue({
        **BASE, "from": "@carla", "to": "@marc", "body": "ten at the cafe?",
        "reply_to": root, "idempotency_key": "k1", "created_at": 200.0,
    })
    store.move(mine, "sent", message_id="message:m2")
    store.update(mine, correlation_id=root)
    theirs = store.receive({
        **BASE, "from": "@marc", "to": "@carla", "body": "four at ours it is",
        "delivery_id": "delivery:d3", "message_id": "message:m3",
        "correlation_id": root, "reply_to": "message:m2", "created_at": 300.0,
    })
    # something else entirely, in its own thread
    store.receive({
        **BASE, "from": "@marc", "to": "@carla", "body": "unrelated",
        "delivery_id": "delivery:d4", "message_id": "message:m4",
        "correlation_id": "message:m4", "created_at": 400.0,
    })

    ids = [m["id"] for m in store.thread(root)]
    assert ids == [inbound, mine, theirs]
    # the bare id resolves the same thread as the prefixed one
    assert [m["id"] for m in store.thread("root")] == ids
    assert store.thread("") == []


def test_thread_finds_mail_written_before_the_server_carried_a_key(store):
    """No correlation_id anywhere: the root itself and a direct answer to it
    still gather under the root's id."""
    root = "message:old"
    a = store.receive({**BASE, "delivery_id": "delivery:d1", "message_id": root,
                       "created_at": 100.0})
    b = store.enqueue({**BASE, "reply_to": root, "idempotency_key": "k1",
                       "created_at": 200.0})
    assert [m["id"] for m in store.thread(root)] == [a, b]


def test_board_mail_round_trips_with_its_board(store):
    """A board post/reply files and parses with `board` intact — and the
    board is part of the content hash, so the same body to two boards (or
    the same board twice after a parse) is never conflated."""
    a = store.enqueue({**BASE, "kind": "board", "to": None, "board": "board:aaa", "tags": None})
    b = store.enqueue({**BASE, "kind": "board", "to": None, "board": "board:bbb", "tags": None})
    assert a != b
    mail = store.get(a)
    assert mail["kind"] == "board"
    assert mail["board"] == "board:aaa"
    # and a delivery filed from a poll carries it too
    d = store.receive(
        {**BASE, "kind": "board", "to": None, "tags": None, "board": "board:aaa",
         "delivery_id": "delivery:9", "message_id": "message:9"}
    )
    assert store.get(d)["board"] == "board:aaa"


def test_board_mail_requires_a_board(store):
    with pytest.raises(ValueError):
        store.enqueue({**BASE, "kind": "board", "to": None, "tags": None})
    with pytest.raises(ValueError):  # single-line identifiers, like the rest
        store.enqueue({**BASE, "kind": "board", "to": None, "tags": None, "board": "two\nlines"})
