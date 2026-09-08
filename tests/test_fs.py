"""Tests for the shared crash-safe filesystem helper (lloom/_fs.py)."""

from __future__ import annotations

import os

import pytest

from lloom._fs import atomic_write_text


def test_atomic_write_creates_file(tmp_path):
    p = tmp_path / "cursor.txt"
    atomic_write_text(p, "abc")
    assert p.read_text(encoding="utf-8") == "abc"
    assert not list(tmp_path.glob(".*.tmp"))


def test_atomic_write_failure_leaves_target_untouched_and_no_tmp(tmp_path, monkeypatch):
    """F9: a failed write (torn/rejected rename) must leave the previous
    content intact (or nothing, for a fresh file) and NO .tmp leftovers —
    a torn cursor.txt would permanently pin the client to empty polls."""

    def boom(src, dst):
        raise OSError("simulated crash before rename")

    monkeypatch.setattr(os, "replace", boom)

    fresh = tmp_path / "cursor.txt"
    with pytest.raises(OSError):
        atomic_write_text(fresh, "new-cursor")
    assert not fresh.exists()
    assert not list(tmp_path.glob(".*.tmp"))

    existing = tmp_path / "existing.txt"
    existing.write_text("old-cursor", encoding="utf-8")
    with pytest.raises(OSError):
        atomic_write_text(existing, "new-cursor")
    assert existing.read_text(encoding="utf-8") == "old-cursor"
    assert not list(tmp_path.glob(".*.tmp"))


def test_atomic_write_replaces_existing_content(tmp_path):
    p = tmp_path / "cursor.txt"
    p.write_text("old", encoding="utf-8")
    atomic_write_text(p, "new")
    assert p.read_text(encoding="utf-8") == "new"
    assert not list(tmp_path.glob(".*.tmp"))


def test_atomic_write_concurrent_writers_do_not_clobber(tmp_path):
    """Review round 10: unique temp names — concurrent writers to the same
    target must all succeed without touching each other's temp files."""
    import concurrent.futures

    target = tmp_path / "state.txt"
    target.write_text("original")

    def write(i: int) -> str:
        atomic_write_text(target, f"content-{i}")
        return target.read_text()

    with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
        results = list(pool.map(write, range(40)))
    assert all(r.startswith("content-") for r in results)
    assert target.read_text().startswith("content-")
    assert not list(tmp_path.glob("*.tmp"))


def test_atomic_write_owner_only_mode(tmp_path):
    import stat

    target = tmp_path / "secret.txt"
    atomic_write_text(target, "private")
    assert stat.S_IMODE(target.stat().st_mode) == 0o600
