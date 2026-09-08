"""Client config: `~/.lloom/config.json` (0600, atomic writes).

All derived client state (cursor, legacy outbox) lives in a `state/` directory
sibling to the config file, so distinct `--config` files never share state.
"""

from __future__ import annotations

import json
import os
import stat
import tempfile
from pathlib import Path
from typing import Any

STATE_DIRNAME = "state"


def default_config_path() -> Path:
    env = os.environ.get("LLOOM_CONFIG")
    if env:
        return Path(env)
    return Path.home() / ".lloom" / "config.json"


class Config:
    def __init__(self, path: Path | str | None = None):
        self.path = Path(path) if path else default_config_path()

    def load(self) -> dict[str, Any]:
        if not self.path.exists():
            return {}
        return json.loads(self.path.read_text())

    def save(self, data: dict[str, Any]) -> None:
        """Atomically write config: tempfile in the same dir, chmod 0600,
        then os.replace() so readers never see a partial file."""
        self.path.parent.mkdir(parents=True, exist_ok=True)
        fd, tmp_name = tempfile.mkstemp(prefix=".config.", dir=self.path.parent)
        try:
            with os.fdopen(fd, "w") as f:
                f.write(json.dumps(data, indent=2))
            os.chmod(tmp_name, stat.S_IRUSR | stat.S_IWUSR)
            os.replace(tmp_name, self.path)
        except BaseException:
            try:
                os.unlink(tmp_name)
            except OSError:
                pass
            raise

    def get(self, key: str, default: Any = None) -> Any:
        return self.load().get(key, default)

    def set(self, key: str, value: Any) -> None:
        data = self.load()
        data[key] = value
        self.save(data)

    def delete(self, key: str) -> None:
        data = self.load()
        data.pop(key, None)
        self.save(data)

    def state_dir(self) -> Path:
        """Derived state directory: `<dir-of-config>/state/<config-filename>/`
        (created on demand). Namespacing by the FULL filename (not the stem)
        keeps distinct configs sharing a stem (`alice.json` vs `alice.dev`)
        from sharing state — a shared cursor.txt would let one profile
        permanently skip the other's deliveries."""
        d = self.path.parent / STATE_DIRNAME / self.path.name
        d.mkdir(parents=True, exist_ok=True)
        return d

    def shared_state_dir(self) -> Path:
        """Pre-isolation layout (`<dir-of-config>/state/`), only probed for
        legacy outbox migration."""
        return self.path.parent / STATE_DIRNAME

    def cursor_path(self) -> Path:
        return self.state_dir() / "cursor.txt"


def ensure_config(path: Path | None = None) -> Config:
    c = Config(path)
    c.path.parent.mkdir(parents=True, exist_ok=True)
    return c
