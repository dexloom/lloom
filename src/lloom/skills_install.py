"""`lloom skills install` — install the bundled skills for six coding agents (P10).

The skills ship INSIDE this package (`lloom/skills/`), so `pip install
lloom-client` is all you need — there is no repo to be standing in. The
repo's `.agents/skills/<name>` entries are symlinks onto these same files,
which is the path Codex, OpenCode, Pi, and OpenClaw discover natively when an
agent is working inside the lloom repo itself; because they are symlinks there
is only ever one copy to edit.

Installing means COPYING the skills to the folder the chosen agent actually
reads — project-wide or system-wide. The three agent-facing skills are
installed (`lloom-setup`, `lloom-send`, `lloom-receive`); `lloom-testenv`
stays repo-only and is not bundled at all:

* **claude-code** — project: copy into `.agents/skills/<name>`, then symlink
  `.claude/skills/<name>` -> `../../.agents/skills/<name>` so one project copy
  serves every agent; global: absolute symlinks in `~/.claude/skills`.
* **codex / opencode / pi / openclaw** — project: copy into the project's
  own `.agents/skills/<name>`, which is where these four discover skills
  natively; global: copy to the tool-NATIVE skills dir (codex
  `$CODEX_HOME/skills` default `~/.codex/skills`;
  opencode `$XDG_CONFIG_HOME/opencode/skills` default
  `~/.config/opencode/skills`; pi `~/.pi/agent/skills`; openclaw
  `~/.openclaw/skills` when that dir exists, else no native copy).
* **hermes** — no native project scope: copy to `~/.hermes/skills` (both scopes).
* **every global install** ALSO refreshes the shared `~/.agents/skills/`
  copy — the cross-tool standard — so skills remain discoverable by tools
  that read that directory regardless of which agent you installed for.

`--with-mcp` prints/applies the agent's MCP registration. Non-destructive:
in a project it may only touch `.agents/skills/`, `.claude/skills/` and the
project-root `.mcp.json`; outside a project the only automated write beyond
the skills copies is the OpenCode global config (`opencode.json` under
`$XDG_CONFIG_HOME/opencode`, JSON-merged preserving existing keys).
Everything else prints a paste-ready snippet (mirrored in
`docs/agents/<agent>.md`).

Idempotent: an identical re-run reports OK everywhere and writes nothing; a
differing destination is reported as SKIP with a warning and never
overwritten (no `--force` — delete the destination to re-install).
"""

from __future__ import annotations

import dataclasses
import importlib.resources
import json
import os
import shutil
import sys
from collections.abc import Callable
from pathlib import Path

AGENTS = ("claude-code", "codex", "opencode", "pi", "hermes", "openclaw")
INSTALLABLE_SKILLS = ("lloom-setup", "lloom-send", "lloom-receive")
# Where a project keeps skills for the natively-discovering agents. Used as a
# DESTINATION now, and as the marker that says "this is the lloom repo".
PROJECT_SKILLS_SUBDIR = Path(".agents") / "skills"
CANONICAL_MARKER = PROJECT_SKILLS_SUBDIR / "lloom-send"


def bundled_skills_dir() -> Path:
    """The skills shipped inside this package — the one source of truth.

    In an editable/dev install this resolves to `src/lloom/skills`, the very
    files the repo's `.agents/skills` symlinks point at, so editing a skill
    in the repo takes effect immediately and no second copy can drift.
    """
    return Path(str(importlib.resources.files("lloom") / "skills"))


# The Agent Skills spec allows six frontmatter fields: name, description,
# and four optional metadata fields. Our skills use only name + description.
ALLOWED_FRONTMATTER_FIELDS = frozenset(
    {"name", "description", "allowed-tools", "model", "license", "metadata"}
)

# Paste-ready MCP registrations (single source of truth: docs/agents/<agent>.md
# under "## MCP registration"; these embedded copies are the fallback and are
# unit-tested to stay identical to the docs).
MCP_SNIPPETS: dict[str, str] = {
    "claude-code": (
        '{\n  "mcpServers": {\n    "lloom": { "command": "lloom", "args": ["mcp-proxy"] }\n  }\n}'
    ),
    "codex": '[mcp_servers.lloom]\ncommand = "lloom"\nargs = ["mcp-proxy"]',
    "opencode": (
        '{\n  "mcp": {\n    "lloom": { "type": "local", "command": ["lloom", "mcp-proxy"], "enabled": true }\n  }\n}'
    ),
    "pi": '{\n  "mcpServers": {\n    "lloom": { "command": "lloom", "args": ["mcp-proxy"] }\n  }\n}',
    "hermes": "mcp_servers:\n  lloom:\n    command: lloom\n    args: [\"mcp-proxy\"]",
    "openclaw": (
        '{\n  "mcp": {\n    "servers": {\n      "lloom": { "command": "lloom", "args": ["mcp-proxy"] }\n    }\n  }\n}'
    ),
}

OPENCODE_MCP_ENTRY = {"type": "local", "command": ["lloom", "mcp-proxy"], "enabled": True}
_MCP_JSON_ENTRY = {"command": "lloom", "args": ["mcp-proxy"]}


# -- frontmatter validation ------------------------------------------------------


def parse_frontmatter(text: str) -> dict[str, str]:
    """Parse a SKILL.md's YAML-ish frontmatter (supports folded `>-` blocks)."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}
    fm: dict[str, str] = {}
    key: str | None = None
    for line in lines[1:]:
        if line.strip() == "---":
            break
        if line[:1] in (" ", "\t") and key is not None:
            fm[key] = (fm[key] + " " + line.strip()).strip()
        elif ":" in line:
            key, _, value = line.partition(":")
            key, value = key.strip(), value.strip()
            fm[key] = "" if value in (">-", ">", "|", "|-") else value
    return fm


def validate_skill_dir(skill_dir: Path) -> list[str]:
    """Return a list of spec violations for one skill directory (empty = valid)."""
    errors: list[str] = []
    md = skill_dir / "SKILL.md"
    if not md.is_file():
        return [f"{skill_dir}: missing SKILL.md"]
    fm = parse_frontmatter(md.read_text())
    if fm.get("name") != skill_dir.name:
        errors.append(f"{skill_dir}: frontmatter name {fm.get('name')!r} != dir name {skill_dir.name!r}")
    illegal = sorted(set(fm) - ALLOWED_FRONTMATTER_FIELDS)
    if illegal:
        errors.append(f"{skill_dir}: illegal frontmatter fields {illegal} (allowed: {sorted(ALLOWED_FRONTMATTER_FIELDS)})")
    if not fm.get("description", "").strip():
        errors.append(f"{skill_dir}: missing description")
    return errors


# -- tool-native global skills dirs ------------------------------------------------
# Probed on the dev box (2026-08-24): codex discovers global skills in
# ~/.codex/skills (populated, incl. a .system/ dir); opencode in
# ~/.config/opencode/skills (a real skill lives there and is loaded);
# pi in ~/.pi/agent/skills (populated); openclaw in ~/.openclaw/skills
# (populated). ~/.agents/skills is the shared cross-tool standard.


def _codex_skills_dir(home: Path) -> Path:
    """Codex global skills: $CODEX_HOME/skills (default ~/.codex/skills)."""
    env = os.environ.get("CODEX_HOME")
    base = Path(env).expanduser() if env else home / ".codex"
    return base / "skills"


def _opencode_config_dir(home: Path) -> Path:
    """OpenCode global config dir: $XDG_CONFIG_HOME/opencode (default
    ~/.config/opencode) — holds both opencode.json and skills/."""
    env = os.environ.get("XDG_CONFIG_HOME")
    base = Path(env).expanduser() if env else home / ".config"
    return base / "opencode"


def _pi_skills_dir(home: Path) -> Path:
    """Pi global skills: ~/.pi/agent/skills."""
    return home / ".pi" / "agent" / "skills"


def _openclaw_skills_dir(home: Path) -> Path | None:
    """OpenClaw global skills, probed: ~/.openclaw/skills when OpenClaw's
    skills dir exists; None otherwise (the shared ~/.agents/skills copy is
    the fallback and is always installed)."""
    native = home / ".openclaw" / "skills"
    return native if native.is_dir() else None


# -- planning ---------------------------------------------------------------------


@dataclasses.dataclass
class Action:
    verb: str  # CREATE | OK | SKIP | MERGE | PRINT
    dest: Path
    detail: str = ""
    apply: Callable[[], None] | None = None

    def line(self) -> str:
        text = f"{self.verb:<7} {self.dest}"
        if self.detail:
            text += f"  ({self.detail})"
        return text


def find_project_root(start: Path | None = None) -> Path | None:
    """Walk up from `start` (default CWD) to the project a --project install targets.

    Prefers a directory that already keeps `.agents/skills/` (the lloom repo, or
    a project that has been installed into before) so running from a subdirectory
    still lands in the right place; otherwise the enclosing git repo. Returns
    None when neither marker is found, and the caller falls back to the CWD —
    installing is now possible anywhere, not only inside the lloom checkout.
    """
    start = (start or Path.cwd()).resolve()
    for marker in (CANONICAL_MARKER, Path(".git")):
        cur = start
        while True:
            if (cur / marker).exists():
                return cur
            parent = cur.parent
            if parent == cur:
                break
            cur = parent
    return None


def _rel_files(root: Path) -> set[Path]:
    return {p.relative_to(root) for p in root.rglob("*") if p.is_file()}


def _trees_identical(src: Path, dst: Path) -> bool:
    src_files, dst_files = _rel_files(src), _rel_files(dst)
    return src_files == dst_files and all(
        (src / f).read_bytes() == (dst / f).read_bytes() for f in src_files
    )


def _copy_action(src: Path, dst: Path, note: str = "") -> Action:
    if dst.is_dir():
        if _trees_identical(src, dst):
            return Action("OK", dst, "identical copy" + (f", {note}" if note else ""))
        return Action("SKIP", dst, "MISMATCH: exists and differs; not overwriting — delete it to re-install" + (f" ({note})" if note else ""))

    def apply() -> None:
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(src, dst)

    return Action("CREATE", dst, f"copy of {src}" + (f", {note}" if note else ""), apply)


def _symlink_action(dst: Path, target: str) -> Action:
    if dst.is_symlink():
        if os.readlink(dst) == target:
            return Action("OK", dst, f"symlink -> {target}")
        return Action("SKIP", dst, f"MISMATCH: symlink points to {os.readlink(dst)!r}, want {target!r}; not overwriting")
    if dst.exists():
        return Action("SKIP", dst, "MISMATCH: exists and is not a symlink; not overwriting")

    def apply() -> None:
        dst.parent.mkdir(parents=True, exist_ok=True)
        os.symlink(target, dst)

    return Action("CREATE", dst, f"symlink -> {target}", apply)


def _mcp_json_action(repo_root: Path) -> Action:
    """claude-code/pi project: the repo-root .mcp.json IS the registration."""
    dest = repo_root / ".mcp.json"
    want = {"mcpServers": {"lloom": dict(_MCP_JSON_ENTRY)}}
    if dest.is_file():
        try:
            if json.loads(dest.read_text()).get("mcpServers", {}).get("lloom") == _MCP_JSON_ENTRY:
                return Action("OK", dest, "repo .mcp.json already registers lloom")
            return Action("SKIP", dest, "MISMATCH: .mcp.json exists without the lloom entry; not overwriting")
        except ValueError:
            return Action("SKIP", dest, "MISMATCH: .mcp.json is not valid JSON; not overwriting")

    def apply() -> None:
        dest.write_text(json.dumps(want, indent=2) + "\n")

    return Action("CREATE", dest, "repo .mcp.json registering lloom mcp-proxy", apply)


def _opencode_merge_action(home: Path) -> Action:
    """--global --with-mcp for OpenCode: JSON-merge the 'mcp' key, preserving all
    existing configuration. The only automated write outside the repo."""
    dest = _opencode_config_dir(home) / "opencode.json"
    existing: dict = {}
    if dest.is_file():
        try:
            existing = json.loads(dest.read_text())
        except ValueError:
            return Action("SKIP", dest, "MISMATCH: not valid JSON; not touching it")
        current = existing.get("mcp", {}).get("lloom")
        if current == OPENCODE_MCP_ENTRY:
            return Action("OK", dest, "mcp.lloom already registered")
        if current is not None:
            return Action("SKIP", dest, "MISMATCH: mcp.lloom exists and differs; not overwriting")

    def apply() -> None:
        merged = dict(existing)
        merged.setdefault("mcp", {})["lloom"] = dict(OPENCODE_MCP_ENTRY)
        dest.parent.mkdir(parents=True, exist_ok=True)
        tmp = dest.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(merged, indent=2) + "\n")
        os.replace(tmp, dest)

    return Action("MERGE", dest, "add mcp.lloom, preserving existing keys", apply)


def _snippet_for(repo_root: Path | None, agent: str) -> str:
    """Prefer the paste-ready snippet from docs/agents/<agent>.md (single source
    of truth); fall back to the embedded copy."""
    if repo_root is not None:
        doc = repo_root / "docs" / "agents" / f"{agent}.md"
        if doc.is_file():
            snippet = _extract_mcp_snippet(doc.read_text())
            if snippet is not None:
                return snippet
    return MCP_SNIPPETS[agent]


def _extract_mcp_snippet(doc: str) -> str | None:
    """First non-bash fenced block under the '## MCP registration' heading."""
    in_section = False
    in_block = False
    block: list[str] = []
    for line in doc.splitlines():
        stripped = line.strip()
        if stripped.startswith("## ") and in_section:
            break
        if stripped == "## MCP registration":
            in_section = True
            continue
        if not in_section:
            continue
        if not in_block and stripped.startswith("```") and stripped != "```bash":
            in_block = True
            block = []
            continue
        if in_block and stripped == "```":
            return "\n".join(block).rstrip("\n")
        if in_block:
            block.append(line)
    return None


# -- install -----------------------------------------------------------------------


def _agent_actions(agent: str, scope: str, project_root: Path, home: Path, with_mcp: bool) -> list[Action]:
    skills_root = bundled_skills_dir()               # source: always the bundled copy
    project_skills = project_root / PROJECT_SKILLS_SUBDIR
    shared_root = home / ".agents" / "skills"
    actions: list[Action] = []

    def project_copies(note: str) -> None:
        """Materialize the skills in the project's own `.agents/skills/`.

        Inside the lloom repo those entries are symlinks onto the bundled
        source, so this compares equal and writes nothing.
        """
        for name in INSTALLABLE_SKILLS:
            actions.append(_copy_action(skills_root / name, project_skills / name, note=note))

    def shared_copies() -> None:
        """The shared ~/.agents/skills copy — cross-tool standard, kept
        fresh by every global install."""
        for name in INSTALLABLE_SKILLS:
            actions.append(
                _copy_action(skills_root / name, shared_root / name, note="shared cross-tool standard")
            )

    if agent == "claude-code":
        if scope == "project":
            # the symlink target has to exist for anyone outside the lloom repo
            project_copies("project skills, shared with the natively-discovering agents")
            dest_base, target_of = project_root / ".claude" / "skills", lambda n: f"../../.agents/skills/{n}"
        else:
            dest_base, target_of = home / ".claude" / "skills", lambda n: str(skills_root / n)
        for name in INSTALLABLE_SKILLS:
            actions.append(_symlink_action(dest_base / name, target_of(name)))
        if scope == "global":
            shared_copies()
    elif agent in ("codex", "opencode", "pi", "openclaw"):
        if scope == "project":
            project_copies(f"{agent} discovers .agents/skills natively")
        else:
            native = {
                "codex": _codex_skills_dir(home),
                "opencode": _opencode_config_dir(home) / "skills",
                "pi": _pi_skills_dir(home),
                "openclaw": _openclaw_skills_dir(home),
            }[agent]
            if native is not None:
                for name in INSTALLABLE_SKILLS:
                    actions.append(
                        _copy_action(skills_root / name, native / name, note=f"{agent} native global")
                    )
            shared_copies()
    elif agent == "hermes":
        note = "hermes has no native project scope — installed globally" if scope == "project" else ""
        for name in INSTALLABLE_SKILLS:
            actions.append(_copy_action(skills_root / name, home / ".hermes" / "skills" / name, note=note))
        if scope == "global":
            shared_copies()

    if with_mcp:
        if agent in ("claude-code", "pi") and scope == "project":
            actions.append(_mcp_json_action(project_root))
        elif agent == "opencode" and scope == "global":
            actions.append(_opencode_merge_action(home))
        else:
            dest = {
                "claude-code": "~/.claude.json (mcpServers)",
                "codex": "~/.codex/config.toml [mcp_servers.lloom]",
                "opencode": "opencode.json (project scope: repo-root opencode.json)",
                "pi": "~/.pi/agent/mcp.json (mcpServers)",
                "hermes": "~/.hermes/config.yaml (mcp_servers)",
                "openclaw": "~/.openclaw/openclaw.json (mcp.servers)",
            }[agent]
            actions.append(Action("PRINT", Path(dest), "paste-ready snippet below", None))
    return actions


def run_install(agent: str, scope: str, with_mcp: bool, dry_run: bool) -> int:
    """Plan + (unless dry-run) apply the install. Returns a process exit code."""
    home = Path.home()
    targets = list(AGENTS) if agent == "all" else [agent]

    # The skills ship with this package, so an install works anywhere; the
    # project root only decides WHERE a --project install writes.
    skills_root = bundled_skills_dir()
    project_root = find_project_root() or Path.cwd()

    if not skills_root.is_dir():
        print(
            f"error: bundled skills missing from the installed package ({skills_root})\n"
            "(this is a packaging fault — please report it)",
            file=sys.stderr,
        )
        return 1

    invalid = [e for name in INSTALLABLE_SKILLS for e in validate_skill_dir(skills_root / name)]
    if invalid:
        for e in invalid:
            print(f"error: {e}", file=sys.stderr)
        return 1

    print(f"source: {skills_root} (bundled)")
    if scope == "project":
        print(f"project: {project_root}")
    exit_code = 0
    for target in targets:
        print(f"\n{target} ({scope}):")
        for action in _agent_actions(target, scope, project_root, home, with_mcp):
            print(f"  {action.line()}")
            if action.verb == "PRINT":
                snippet = _snippet_for(project_root, target)
                for line in snippet.splitlines():
                    print(f"      {line}")
            if not dry_run and action.apply is not None:
                action.apply()
    if dry_run:
        print("\n(dry-run: nothing was written)")
    return exit_code
