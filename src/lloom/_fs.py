"""Shared crash-safe filesystem helpers (client)."""

from __future__ import annotations

import os
import uuid
from pathlib import Path


def fsync_dir(path: Path | str) -> None:
    """fsync a directory so a just-created/renamed entry survives a host
    crash (fsyncing file contents alone does not persist the name)."""
    fd = os.open(Path(path), os.O_RDONLY)
    try:
        os.fsync(fd)
    except OSError:
        pass  # some filesystems refuse directory fsync; best effort
    finally:
        os.close(fd)


def atomic_write_text(path: Path | str, text: str, mode: int = 0o600) -> None:
    """Crash-safe text write: unique temp file in the same folder,
    flush+fsync, then atomic ``os.replace`` (same filesystem) so readers
    never see a partial file under the final name, and an fsync of the
    parent directory so the new name itself survives a host crash. The
    unique temp name keeps concurrent writers to the same target from
    clobbering each other's temp files; the temp is removed if any step
    fails — a failed write leaves no ``.tmp`` leftovers and the previous
    content (if any) untouched.
    """
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.{uuid.uuid4().hex}.tmp")
    try:
        # newline="": no translation — "\n" stays "\n" (and "\r" untouched)
        # on every platform, preserving byte-exact mail file contents
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
        # respect a restrictive umask only for widening: never more permissive
        os.fchmod(fd, mode)
        with os.fdopen(fd, "w", encoding="utf-8", newline="") as fh:
            fh.write(text)
            fh.flush()
            os.fsync(fh.fileno())
        os.replace(tmp, path)
        fsync_dir(path.parent)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise
