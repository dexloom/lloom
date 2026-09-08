"""`lloom skills install` unit tests: fake HOME + fake project dir, plus
frontmatter validation of the skills bundled inside the package.

The skills ship in `lloom/skills/`; the repo's `.agents/skills/<name>` are
symlinks onto them. So the installer's SOURCE is always the package, and a
project dir is only ever a DESTINATION — which is what makes the command work
for someone who installed from PyPI and has no lloom checkout."""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path

import pytest

from lloom import cli
from lloom.skills_install import (
    AGENTS,
    ALLOWED_FRONTMATTER_FIELDS,
    INSTALLABLE_SKILLS,
    MCP_SNIPPETS,
    _extract_mcp_snippet,
    bundled_skills_dir,
    find_project_root,
    parse_frontmatter,
    validate_skill_dir,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
SKILLS_DIR = REPO_ROOT / ".agents" / "skills"   # repo-root discovery aliases
BUNDLED = bundled_skills_dir()                  # the single source of truth


def _install(capsys, *argv: str) -> tuple[int, str]:
    try:
        cli.main(["skills", "install", *argv])
    except SystemExit as exc:
        code = exc.code
    else:  # pragma: no cover - cmd_skills always exits or returns
        code = 0
    out = capsys.readouterr().out
    assert isinstance(code, int), f"exit code not an int: {code!r}"
    return code, out


@pytest.fixture
def fake_home(tmp_path, monkeypatch):
    home = tmp_path / "home"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    # native-dir resolution must not leak the developer's real overrides
    monkeypatch.delenv("CODEX_HOME", raising=False)
    monkeypatch.delenv("XDG_CONFIG_HOME", raising=False)
    return home


@pytest.fixture
def fake_project(tmp_path):
    """A destination project holding no skills of its own — the PyPI-user case.

    `.git` is the marker `find_project_root` walks up to once a project has no
    `.agents/skills/` yet, which is exactly the state a first install starts from.
    """
    proj = tmp_path / "proj"
    (proj / ".git").mkdir(parents=True)
    (proj / ".mcp.json").write_text(
        json.dumps({"mcpServers": {"lloom": {"command": "lloom", "args": ["mcp-proxy"]}}})
    )
    return proj


def _snapshot(*roots: Path) -> dict:
    snap: dict[str, object] = {}
    for root in roots:
        for path in sorted(root.rglob("*")):
            rel = str(path.relative_to(root))
            if path.is_symlink():
                snap[f"{root.name}:{rel}"] = ("link", os.readlink(path))
            elif path.is_file():
                snap[f"{root.name}:{rel}"] = ("file", hashlib.sha256(path.read_bytes()).hexdigest())
            else:
                snap[f"{root.name}:{rel}"] = ("dir", None)
    return snap


# -- plan / dry-run -----------------------------------------------------------------


def test_dry_run_shows_six_agent_plan_and_writes_nothing(fake_home, fake_project, monkeypatch, capsys):
    monkeypatch.chdir(fake_project)
    code, out = _install(capsys, "--agent", "all", "--dry-run")
    assert code == 0
    for agent in AGENTS:
        assert agent in out, f"agent {agent} missing from the plan"
    assert out.count("lloom-setup") >= len(AGENTS)
    assert "CREATE" in out and ".claude/skills" in out
    assert "dry-run" in out
    # the GLOBAL plan names each tool's NATIVE global dir and the shared standard
    code, gout = _install(capsys, "--agent", "all", "--global", "--dry-run")
    assert code == 0
    assert ".codex/skills" in gout
    assert ".config/opencode/skills" in gout
    assert ".pi/agent/skills" in gout
    assert ".agents/skills" in gout and "shared cross-tool standard" in gout
    assert "dry-run" in gout
    # nothing written anywhere
    assert not (fake_project / ".claude").exists()
    assert not (fake_home / ".hermes").exists()
    assert not (fake_home / ".agents").exists()
    assert not (fake_home / ".codex").exists()
    assert not any(fake_home.iterdir())


def test_install_outside_any_repo_uses_the_bundled_skills(fake_home, tmp_path, monkeypatch, capsys):
    """The PyPI case: no lloom checkout above the CWD, and it still installs.

    This used to be a hard error ("run from inside the lloom repo"), which made
    the command dead for everyone who installed from PyPI.
    """
    elsewhere = tmp_path / "elsewhere"
    elsewhere.mkdir()
    monkeypatch.chdir(elsewhere)
    code, out = _install(capsys, "--agent", "claude-code")
    assert code == 0
    assert "(bundled)" in out
    for name in INSTALLABLE_SKILLS:
        installed = elsewhere / ".agents" / "skills" / name / "SKILL.md"
        assert installed.is_file(), f"{name} not installed into the CWD project"
        assert installed.read_text() == (BUNDLED / name / "SKILL.md").read_text()
        assert (elsewhere / ".claude" / "skills" / name).is_symlink()


# -- project scope --------------------------------------------------------------------


def test_project_install_all(fake_home, fake_project, monkeypatch, capsys):
    monkeypatch.chdir(fake_project)
    code, out = _install(capsys, "--agent", "all")
    assert code == 0
    # claude-code: relative symlinks
    for name in INSTALLABLE_SKILLS:
        link = fake_project / ".claude" / "skills" / name
        assert link.is_symlink(), f"{link} not a symlink"
        assert os.readlink(link) == f"../../.agents/skills/{name}"
        assert (link / "SKILL.md").is_file()
    # codex/opencode/pi/openclaw: real copies into the project's own .agents/skills,
    # which is the directory those four discover natively
    assert "discovers .agents/skills natively" in out
    for name in INSTALLABLE_SKILLS:
        landed = fake_project / ".agents" / "skills" / name / "SKILL.md"
        assert landed.is_file(), f"{name} not materialized in the project"
        assert landed.read_text() == (BUNDLED / name / "SKILL.md").read_text()
    # a project install writes nothing into HOME
    assert not (fake_home / ".agents").exists()
    # hermes: copied even in project mode (no native project scope)
    for name in INSTALLABLE_SKILLS:
        copied = fake_home / ".hermes" / "skills" / name / "SKILL.md"
        assert copied.is_file()
        assert copied.read_text() == (BUNDLED / name / "SKILL.md").read_text()
    # lloom-testenv is repo-only: never installed anywhere
    assert not (fake_project / ".claude" / "skills" / "lloom-testenv").exists()
    assert not (fake_home / ".hermes" / "skills" / "lloom-testenv").exists()


def test_second_run_is_a_noop(fake_home, fake_project, monkeypatch, capsys):
    monkeypatch.chdir(fake_project)
    code, _ = _install(capsys, "--agent", "all")
    assert code == 0
    before = _snapshot(fake_project, fake_home)
    code, out = _install(capsys, "--agent", "all")
    assert code == 0
    assert "CREATE" not in out
    assert "MISSING" not in out
    assert out.count("OK") >= len(AGENTS)
    assert _snapshot(fake_project, fake_home) == before


def test_differing_destination_is_skipped_not_overwritten(fake_home, fake_project, monkeypatch, capsys):
    monkeypatch.chdir(fake_project)
    code, _ = _install(capsys, "--agent", "hermes")
    assert code == 0
    target = fake_home / ".hermes" / "skills" / "lloom-send" / "SKILL.md"
    original = target.read_text()
    target.write_text("---\nname: lloom-send\ndescription: local fork\n---\nchanged\n")
    code, out = _install(capsys, "--agent", "hermes")
    assert code == 0
    assert "SKIP" in out and "MISMATCH" in out
    assert target.read_text() == "---\nname: lloom-send\ndescription: local fork\n---\nchanged\n"
    assert original != target.read_text()


# -- global scope ----------------------------------------------------------------------


def test_global_install(fake_home, fake_project, monkeypatch, capsys):
    monkeypatch.chdir(fake_project)
    code, out = _install(capsys, "--agent", "all", "--global")
    assert code == 0
    # tool-NATIVE global dirs, probed per tool:
    for name in INSTALLABLE_SKILLS:
        # codex: $CODEX_HOME/skills (default ~/.codex/skills)
        assert (fake_home / ".codex" / "skills" / name / "SKILL.md").is_file()
        # opencode: $XDG_CONFIG_HOME/opencode/skills (default ~/.config/opencode/skills)
        assert (fake_home / ".config" / "opencode" / "skills" / name / "SKILL.md").is_file()
        # pi: ~/.pi/agent/skills
        assert (fake_home / ".pi" / "agent" / "skills" / name / "SKILL.md").is_file()
        # hermes: ~/.hermes/skills
        assert (fake_home / ".hermes" / "skills" / name / "SKILL.md").is_file()
        # EVERY global install also keeps the shared ~/.agents/skills copy
        assert (fake_home / ".agents" / "skills" / name / "SKILL.md").is_file()
    # openclaw: no ~/.openclaw/skills in the fake home -> shared fallback only
    assert not (fake_home / ".openclaw").exists()
    # claude-code: absolute symlinks in ~/.claude/skills (+ shared copy above)
    for name in INSTALLABLE_SKILLS:
        link = fake_home / ".claude" / "skills" / name
        assert link.is_symlink()
        assert Path(os.readlink(link)).is_absolute()
        # points at the package, not at whichever project was the CWD — so
        # `pip install -U lloom-client` refreshes them in place, and they do not
        # dangle when a checkout is moved or deleted
        assert os.readlink(link) == str(BUNDLED / name)
    assert not (fake_project / ".claude").exists()  # global never touches the project
    assert "lloom-testenv" not in out


def test_global_openclaw_native_dir_when_probe_hits(fake_home, fake_project, monkeypatch, capsys):
    """openclaw probes ~/.openclaw/skills: when it exists (OpenClaw home
    present), the native copy lands there alongside the shared one."""
    monkeypatch.chdir(fake_project)
    (fake_home / ".openclaw" / "skills").mkdir(parents=True)
    code, out = _install(capsys, "--agent", "openclaw", "--global")
    assert code == 0
    assert (fake_home / ".openclaw" / "skills" / "lloom-send" / "SKILL.md").is_file()
    assert (fake_home / ".agents" / "skills" / "lloom-send" / "SKILL.md").is_file()
    assert "openclaw native global" in out


def test_global_env_overrides_honored(fake_home, fake_project, monkeypatch, capsys):
    """CODEX_HOME and XDG_CONFIG_HOME relocate the codex/opencode native
    global dirs (and the opencode --with-mcp merge target)."""
    monkeypatch.chdir(fake_project)
    codex_home = fake_home / "custom-codex"
    xdg = fake_home / "custom-xdg"
    monkeypatch.setenv("CODEX_HOME", str(codex_home))
    monkeypatch.setenv("XDG_CONFIG_HOME", str(xdg))
    code, _ = _install(capsys, "--agent", "codex", "--global")
    assert code == 0
    assert (codex_home / "skills" / "lloom-send" / "SKILL.md").is_file()
    assert not (fake_home / ".codex").exists()
    code, _ = _install(capsys, "--agent", "opencode", "--global", "--with-mcp")
    assert code == 0
    assert (xdg / "opencode" / "skills" / "lloom-send" / "SKILL.md").is_file()
    merged = json.loads((xdg / "opencode" / "opencode.json").read_text())
    assert merged["mcp"]["lloom"]["command"] == ["lloom", "mcp-proxy"]
    # the shared copy still lands under HOME regardless of the overrides
    assert (fake_home / ".agents" / "skills" / "lloom-send" / "SKILL.md").is_file()


# -- with-mcp ---------------------------------------------------------------------------


def test_with_mcp_opencode_global_merges_preserving_existing(fake_home, fake_project, monkeypatch, capsys):
    monkeypatch.chdir(fake_project)
    cfg = fake_home / ".config" / "opencode" / "opencode.json"
    cfg.parent.mkdir(parents=True)
    cfg.write_text(json.dumps({"theme": "dark", "mcp": {"other": {"type": "local", "command": ["x"]}}}))
    code, _ = _install(capsys, "--agent", "opencode", "--global", "--with-mcp", "--dry-run")
    assert code == 0
    assert json.loads(cfg.read_text())["mcp"] == {"other": {"type": "local", "command": ["x"]}}  # dry-run: untouched
    code, out = _install(capsys, "--agent", "opencode", "--global", "--with-mcp")
    assert code == 0
    merged = json.loads(cfg.read_text())
    assert merged["theme"] == "dark"  # preserved
    assert merged["mcp"]["other"] == {"type": "local", "command": ["x"]}  # preserved
    assert merged["mcp"]["lloom"]["command"] == ["lloom", "mcp-proxy"]
    code, out = _install(capsys, "--agent", "opencode", "--global", "--with-mcp")  # idempotent
    assert code == 0 and "MERGE" not in out


def test_with_mcp_prints_snippets_for_other_agents(fake_home, fake_project, monkeypatch, capsys):
    monkeypatch.chdir(fake_project)
    code, out = _install(capsys, "--agent", "codex", "--with-mcp")
    assert code == 0
    assert "PRINT" in out and "mcp_servers.lloom" in out and "config.toml" in out
    # repo .mcp.json untouched (print-only outside the opencode-global merge)
    code, out = _install(capsys, "--agent", "claude-code", "--with-mcp")
    assert code == 0
    assert ".mcp.json" in out and "already registers" in out
    assert json.loads((fake_project / ".mcp.json").read_text())["mcpServers"]["lloom"]["command"] == "lloom"


# -- canonical repo skills: frontmatter + snippet parity ---------------------------------


def test_repo_skills_frontmatter_valid():
    skills = sorted(p for p in SKILLS_DIR.iterdir() if p.is_dir())
    assert [p.name for p in skills] == [
        "lloom-receive",
        "lloom-send",
        "lloom-setup",
    ]
    for skill in skills:
        assert validate_skill_dir(skill) == [], validate_skill_dir(skill)


def test_bundled_skills_ship_inside_the_package():
    """The packaging guard.

    `lloom skills install` was dead for PyPI users because the skills lived
    outside `src/lloom` and never reached the wheel. If someone moves them back
    out, this fails in CI rather than in a published release.
    """
    assert BUNDLED.is_dir(), f"bundled skills missing: {BUNDLED}"
    assert BUNDLED.name == "skills" and BUNDLED.parent.name == "lloom"
    for name in INSTALLABLE_SKILLS:
        assert (BUNDLED / name / "SKILL.md").is_file(), f"{name} not bundled"
        assert validate_skill_dir(BUNDLED / name) == []
    # repo-only, and deliberately not shipped to users
    assert not (BUNDLED / "lloom-testenv").exists()


def test_repo_agents_skills_are_symlinks_onto_the_bundled_copy():
    """One copy, two paths: the repo alias must not become a divergent fork."""
    for name in INSTALLABLE_SKILLS:
        alias = SKILLS_DIR / name
        assert alias.is_symlink(), f"{alias} should be a symlink into the package"
        assert alias.resolve() == (BUNDLED / name).resolve()
    # every alias is accounted for: no stray directory beside the three symlinks
    assert sorted(p.name for p in SKILLS_DIR.iterdir()) == sorted(INSTALLABLE_SKILLS)


def test_repo_installer_skills_are_the_installable_three():
    assert set(INSTALLABLE_SKILLS) == {"lloom-setup", "lloom-send", "lloom-receive"}
    assert "lloom-testenv" not in INSTALLABLE_SKILLS


def test_frontmatter_validation_catches_illegal_fields(tmp_path):
    bad = tmp_path / "lloom-bad"
    bad.mkdir()
    (bad / "SKILL.md").write_text(
        "---\nname: wrong-name\nversion: 1.2.3\ndescription: x\n---\nbody\n"
    )
    errors = validate_skill_dir(bad)
    assert any("wrong-name" in e for e in errors)
    assert any("version" in e for e in errors)
    assert ALLOWED_FRONTMATTER_FIELDS >= {"name", "description"}  # spec: 6 fields max


def test_parse_frontmatter_folds_multiline_description():
    fm = parse_frontmatter(
        "---\nname: x\ndescription: >-\n  one two\n  three.\nother: plain\n---\nbody\n"
    )
    assert fm["name"] == "x"
    assert fm["description"] == "one two three."
    assert fm["other"] == "plain"


def test_docs_snippets_match_embedded_ones():
    for agent in AGENTS:
        doc = (REPO_ROOT / "docs" / "agents" / f"{agent}.md").read_text()
        extracted = _extract_mcp_snippet(doc)
        assert extracted == MCP_SNIPPETS[agent], f"docs/agents/{agent}.md snippet drifted from the installer"


def test_find_project_root_from_subdirectory(fake_project, monkeypatch):
    sub = fake_project / "client" / "src"
    sub.mkdir(parents=True)
    monkeypatch.chdir(sub)
    assert find_project_root() == fake_project.resolve()


def test_main_wiring_rejects_unknown_agent(fake_home, fake_project, monkeypatch):
    monkeypatch.chdir(fake_project)
    with pytest.raises(SystemExit):
        cli.main(["skills", "install", "--agent", "vscode"])


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-q"]))
