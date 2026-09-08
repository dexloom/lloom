"""Config tests: atomic 0600 save, LLOOM_CONFIG honored, state dir isolation."""

from __future__ import annotations

import json
import stat

from lloom.config import Config, ensure_config


def test_save_is_atomic_0600_no_leftovers(tmp_path):
    cfg = Config(tmp_path / "cfg.json")
    cfg.save({"api_key": "llm_secret"})
    path = tmp_path / "cfg.json"
    assert path.exists()
    mode = stat.S_IMODE(path.stat().st_mode)
    assert mode == 0o600
    assert json.loads(path.read_text()) == {"api_key": "llm_secret"}
    # no tempfile leftovers in the config dir
    leftovers = [p.name for p in tmp_path.iterdir() if p.name != "cfg.json"]
    assert leftovers == []


def test_save_replaces_existing_atomically(tmp_path):
    cfg = Config(tmp_path / "cfg.json")
    cfg.save({"a": 1})
    cfg.set("b", 2)
    assert cfg.load() == {"a": 1, "b": 2}
    assert len(list(tmp_path.iterdir())) == 1  # still only the config file


def test_lloom_config_env_honored(tmp_path, monkeypatch):
    env_cfg = tmp_path / "alt.json"
    env_cfg.write_text(json.dumps({"handle": "@env-agent"}))
    monkeypatch.setenv("LLOOM_CONFIG", str(env_cfg))
    cfg = Config()
    assert cfg.path == env_cfg
    assert cfg.get("handle") == "@env-agent"
    cfg.set("x", 1)
    assert json.loads(env_cfg.read_text())["x"] == 1


def test_explicit_path_beats_env(tmp_path, monkeypatch):
    monkeypatch.setenv("LLOOM_CONFIG", str(tmp_path / "env.json"))
    cfg = Config(tmp_path / "explicit.json")
    assert cfg.path == tmp_path / "explicit.json"


def test_state_dir_is_sibling_of_config(tmp_path):
    cfg = ensure_config(tmp_path / "sub" / "config.json")
    state = cfg.state_dir()
    assert state == tmp_path / "sub" / "state" / cfg.path.name
    assert state.is_dir()
    assert cfg.cursor_path() == state / "cursor.txt"
    # per-config stem: two configs in the SAME dir never share state
    other = Config(tmp_path / "sub" / "other.json")
    assert other.state_dir() == tmp_path / "sub" / "state" / "other.json"


def test_two_configs_have_disjoint_state_dirs(tmp_path):
    a = ensure_config(tmp_path / "a" / "config.json")
    b = ensure_config(tmp_path / "b" / "config.json")
    a.cursor_path().write_text("cursor-a")
    assert a.cursor_path().read_text() == "cursor-a"
    assert not b.cursor_path().exists() or b.cursor_path().read_text() != "cursor-a"
