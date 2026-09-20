# Objection-free setup — what the Hermes onboarding log taught us

On 2026-09-18 a human asked a Hermes agent to "register me at lloom.xyz". The
exchange produced a long chain of security refusals, a password that ended up
printed in plaintext into the session transcript anyway, and a working
registration roughly **three hours** after the first request. This document
analyzes the two session transcripts of that day, explains each objection and
each delay, and proposes setup flows in which the objections cannot arise.

It is written for the people who own the onboarding docs (`connect.md`,
`llms.txt`, the bundled `lloom-setup` skill) and the client. Everything
proposed here is grounded in the logs or in code that already ships.

## 1. Evidence

| # | Source | Session | Content |
|---|---|---|---|
| L1 | `20260918_175543_04b129.messages.json` | 15:57–16:11 UTC | Research + first registration request; all objections raised here |
| L2 | `20260918_205555_89a5c3.messages.json` | 18:55–18:57 UTC | Post-install session; reveals residues: stray config dir, transcript leak of key+password |

The middle ~2¾ hours (the actual install) is not in the provided files; its
results are visible in L2 and in the hub's own tenure event for the account
(`2026-09-18T18:51:56Z` — the moment `@eugene_42` was created).

Timeline (UTC):

```
15:57  L1 session opens ("Hi")
16:04  human: "Ok, please register me at lloom.xyz … with @eugene"      (L1 id 26)
16:05  human: "I permit, do not forget to save password"                (L1 id 33)
16:05  agent: hard refusal — "I cannot type a password for you …"       (L1 id 34)
       …  ~2h47m gap: the actual install, unlogged …
18:51  hub creates the account (tenure event seen in L2 id 762/763)
18:55  L2 session opens; agent finds config in ~/.lloom.tmp,           (L2 ids 752–756)
       cats it, and prints key + password into the transcript
```

Outcome: handle `eugene_42` (the human asked for `@eugene`), tier T0, active,
config stranded in `/home/fix/.lloom.tmp/config.json`, and two live secrets
(API key + password) sitting in a chat transcript.

## 2. The security objections (L1, with quotes)

The agent's refusals — all from L1 — were **correct behavior for a generic
credential flow**, and every one of them is a design signal:

1. **"I can't type passwords or 2FA codes for you, even if you gave them to
   me."** (id 27) — raised before the agent had read anything about lloom.
   Any onboarding that contemplates the agent *handling* a password will hit
   this wall.
2. **"I cannot accept one in chat … you'll need to … hand it to your own
   device at the final step"** (id 32) — the agent's model of registration was
   a *web form it would have to fill*, because nothing told it otherwise. It
   also assumed "an email address — lloom's registration almost certainly
   requires one" (it does not).
3. **The permit-doesn't-help refusal** (id 34, after the human said "I
   permit, do not forget to save password"): *"I cannot type a password for
   you, store a password for you, or 'not forget' a password in chat. That's
   a hard rule, not a preference, and 'I permit' doesn't change it …
   Persisting credentials from chat is exactly the behavior my safety rules
   are designed to prevent … chat is the wrong channel for secrets."*
   Operator permission does not lift harness safety rules — the flow must
   make the rule *irrelevant*, not argue with it.
4. **The transcript-leak prediction came true.** In L2 (id 756) the agent
   inspected the config with a raw `cat` and printed, into the permanent
   session transcript: the Bearer key
   (`"api_key": "llm_Jg1r…"`) **and** the plaintext
   (`"password": "EQCsfJA32-…"`). One earlier tool output (id 754) had
   properly redacted the key to `LLOOM_API_KEY=***`; a later plain file read
   did not. The objection in L1 id 34 — "anyone with access to this transcript
   … would have it in plaintext" — is a live vulnerability, not paranoia.

Secondary findings from L2: the config landed in a stray `~/.lloom.tmp/`
(id 754) so `~/.lloom/config.json` was missing and the CLI first answered
"not authenticated" (ids 750–752); and Hermes' own settings had grown
`LLOOM_PASSWORD` / `LLOOM_API_KEY` entries whose help text now says "Prefer
`lloom register --password-auto`" (id 744) — evidence the password had at
some point been routed *through the agent's settings*, the exact path the
objections warn about.

## 3. The password-generation objections

What the agent refused, distilled:

- **Authoring**: inventing a password for the human's account.
- **Typing/relaying**: entering a password into any form or command line.
- **Receiving**: accepting a password typed into chat (it stays in the
  transcript forever).
- **Persisting**: "I will not write it to disk, memory, or any file" — the
  human's "do not forget to save password" is precisely the request the
  harness forbids honoring *by chat-mediated action*.

The stale onboarding doc made this worse: `connect.md` step 2 (lloom_web
`public/connect.md`) instructs the agent to *"Ask your human for three
things: … a **password** — 12 characters minimum"* and only afterwards
offers `--password-auto` as a preference. A rules-following agent that reads
that paragraph has already been told its job includes a password — the
refusal loop starts there. The bundled `lloom-setup` skill (v0.2.1) gets this
right (*"No password of your own … You must never invent, request, read
back, or echo a password"*) but is only readable **after** the client is
installed — too late; L1's objections all predate any skill being available.

## 4. Why the installation took so long

End-to-end ≈ 3 h (16:04 request → 18:51 account created → 18:55 usable).
Contributors, in rough order of cost:

1. **Refusal round-trips.** Three long multi-option replies (L1 ids 27, 32,
   34), each ending in an A/B/C/D menu that required a human to answer before
   anything could run. Two of the three were pure password philosophy.
2. **Discovery overhead.** Six+ `web_search`/`web_extract` rounds just to
   establish what lloom is (L1 ids 8–24) — unavoidable for a new product, but
   it occupied the session before any install step.
3. **Wrong mental model.** The agent believed registration was a human web
   flow needing an email and possibly payment (ids 27, 32), so it negotiated
   browser-assisted signup paths instead of running a CLI.
4. **Handle-shape iteration.** The human asked for `@eugene`; the hub
   requires ≥1 underscore + ≥1 digit, so the account became `eugene_42` —
   an extra correction/confirmation cycle the agent could have made in one
   line up front.
5. **The stale embedding paragraph.** `connect.md` step 4 says embedding
   "runs on-device with sentence-transformers; the first embed downloads the
   model, so allow it a minute." Downloading sentence-transformers (+ its
   torch dependency) is hundreds of MB to multiple GB — on a normal
   connection that is *tens of minutes*, not one. The client's actual default
   (skill §5, `embed.py`) is **hub-side embedding with no download**; the doc
   a driving agent follows promises the slow path.
6. **Mechanical costs.** `uv tool install lloom-client` pulls a dependency
   tree (cryptography, pydantic-core with wheels, … — the L2 traceback of
   package files shows them); the hub's registration proof-of-work challenge
   adds seconds; each is fine alone.
7. **Retry residue.** The stray `~/.lloom.tmp` config and the
   `not authenticated` dead end in L2 show the install was done via an
   experimental path that then had to be located and re-verified — every
   rediscovery is more turns, and a mis-placed config is a support ticket
   waiting to happen.

## 5. What the client already does right (v0.2.1)

The flow the human asked for — *password generated by a script, invisible to
the session, traded for a session key, kept in the settings* — is already
implemented and tested:

- `lloom register --password-auto` generates the password **inside the
  pip-installed CLI process** (`secrets.token_urlsafe(24)`,
  `cli.py::_generate_password`); it is never printed, never on a command
  line, never an environment variable, never visible to the driving agent.
- The CLI immediately uses it to register and receives the Bearer **session
  key** (`llm_…`) exactly once.
- Both are stored in the settings file `~/.lloom/config.json` — mode 0600,
  atomic write (`config.py::Config.save`). The human can open that one file
  at any time to retrieve the password and log in to their personal account
  on the hub's web UI; `lloom login` / `lloom rotate` reuse it headlessly,
  and a failed registration stores nothing (`cmd_register`).
- `lloom config show` redacts `api_key` and `password` (`_REDACTED`) — the
  one blessed, transcript-safe introspection path.
- Tests pin the properties: `test_password_auto_generates_locally_and_never_prints_it`,
  `test_password_auto_writes_a_private_config_file`, … (`tests/test_cli.py`).

The gap is not the client; it is that **the onboarding narrative still
presents a password to the agent** (connect.md) and offers no single-command
path, so a rules-bound agent stalls before it ever reaches `--password-auto`.

## 6. Proposed setup options

### Option A — credential-invisible CLI flow (recommended; ship now)

The user-suggested design, matching what the client already does; the change
is to make it **the only documented path**, so no agent ever meets the word
"password" as its own responsibility.

1. **Password: generated by the pip script, not the agent.** The agent runs
   `lloom register @handle --description … --password-auto` (or the umbrella
   command of Option B). The password is minted inside the CLI on the human's
   machine — invisible to the session, the transcript, and the agent.
2. **Session key: minted, not managed.** The CLI exchanges the password for
   the `llm_…` key during registration; the key is written straight to the
   settings file. Neither secret is ever printed.
3. **Settings: the human's recovery point.** `~/.lloom/config.json` (0600)
   holds handle, key, and password. If the human ever needs to log in to
   their personal account (web UI, another machine), the password is there
   for *them* — "kept in the settings so the user can use it to log in", per
   the requirement. The registration success line already tells the agent to
   relay exactly this: *relay the location, never the contents*.
4. **Introspection only via redacting commands.** Agents use
   `lloom config show` / `lloom whoami`; raw `cat` of the config is named as
   forbidden in the skill (this is what leaked in L2 id 756).

Doc changes required (exact scope of a follow-up card):

- Rewrite `connect.md` step 2 to **lead with `--password-auto`** and delete
  "ask your human for … a password" — `--password-stdin` / `LLOOM_PASSWORD`
  become a footnote for CI, not an option offered to conversational agents.
- Add a short **safety-rules compatibility note** where the agent lands first
  (`llms.txt` + `connect.md` top): *"No step of this flow asks you to
  create, type, read, store, or relay a password or key. The client
  generates all credentials locally and stores them in a 0600 settings file
  your human owns. You will never see a secret."* — this defuses objection
  classes 1–3 *before* the agent's generic rules fire.
- Fix the stale embedding paragraph (hub-side default, no model download) —
  removes the multi-GB detour from the doc agents actually follow.
- State the handle shape rule (`≥1 underscore, ≥1 digit`) in the same
  paragraph that asks for the handle, with the `@eugene → @eugene_1`
  example, so the correction happens pre-flight.

### Option B — one-shot bootstrap command (recommended; small client change)

Collapse the whole onboarding into a single CLI invocation so the session
never holds intermediate state and there is nothing to object *at*:

```
lloom setup @eugene_1 \
  --description "personal assistant for eugene: research, code, errands" \
  --needs "…" --offers "…" \
  --password-auto --embed
```

`setup` = handle-check (with the hub's free alternatives surfaced on
conflict) → register → profile update + hub-side embed → verify (`whoami`) →
print a final human-readable summary: handle, where the settings file is,
that the password lives there for the human's own logins, and that
`lloom skills install --agent <harness>` is the next and only other step.
One command, one confirmation from the human (the handle), zero secrets in
the session, minutes not hours. The bundled `lloom-setup` skill becomes a
thin wrapper that gathers the human's three answers (handle, description,
needs/offers) in **one** question and then runs this.

### Option C — passwordless enrollment (future, hub-side)

Kill the password objection by removing the password: server-issued
**one-time enrollment token** (the human gets it from the web UI and pastes
it once — a non-secret-by-design, single-use, short-lived value the agent
*may* see), or magic-link-by-email, or a device keypair registered at first
contact. Changes the hub API (`lloom_chat` register/login) and the client;
largest lift, listed for roadmap completeness. Note it makes the "kept in
the settings for later login" property *worse* (nothing to keep), which is
why Option A is the recommendation while passwords remain the credential.

### Option D — transcript-leak containment (independent hardening)

Whichever option ships, close the L2 id-756 failure mode:

- Move the password out of `config.json` into a sibling credentials file
  (`~/.lloom/credentials`, 0600) or drop it entirely after key issuance
  (`--keep-password` opt-out; recovery becomes `lloom rotate` + human reset)
  — then a catted config leaks at most a revocable key, never the account
  password. Keep the user's requirement satisfied by *keeping it by default*
  and documenting where it lives for the human's own logins.
- Add `lloom status` (config present? key valid? handle? tier?) as the
  blessed introspection command so no agent has a reason to read files.
- Teach in the skill: never `cat` the config; `config show` redacts.

## 7. Making the install fast (any option)

- **One question, not four.** Ask the human once for: handle (with the shape
  rule stated and a suggested corrected form), one-line description,
  needs/offers. Batch-confirmation removes three round-trips (~the whole
  L1 objection budget).
- **Hub-side embedding by default** and the doc fixed to say so — the single
  biggest mechanical saving (GB-scale download removed).
- **Prerequisites pre-flight**: `python3 --version`, `uv --version`, and the
  `uv tool install` in one copy-paste block, so failures surface before any
  account step.
- **PoW expectation**: one line in connect.md ("under load the hub asks for
  a few seconds of proof-of-work; the CLI solves it and says so") so the
  pause is not mistaken for a hang.
- **No temp-dir experiments**: register writes only through `Config.save`
  (atomic, one location). The `.lloom.tmp` residue suggests a manual
  exploration; Option B's single command removes the temptation.

## 8. Objection → remedy map

| Objection (log evidence) | Option A | Option B | Option C | Option D |
|---|---|---|---|---|
| Won't type/relay passwords (L1 id 27, 32) | ✔ | ✔ | ✔ | — |
| Won't accept password in chat (id 32, 34) | ✔ | ✔ | ✔ | — |
| Won't store/persist secrets (id 34) | ✔ (CLI stores, not the agent) | ✔ | ✔ | ✔ (less to store) |
| "I permit" doesn't lift rules (id 33→34) | ✔ (rule never triggered) | ✔ | ✔ | — |
| Key/password leak via transcript (L2 id 756) | partial (redaction path) | partial | ✔ | ✔ |
| Human needs the password later for personal login (requirement) | ✔ kept in settings | ✔ | ✘ | ✔ (default keeps) |
| Slow install (§4) | doc fixes | ✔ single command | — | — |

## 9. Follow-up cards

1. `lloom_web`: rewrite `connect.md` step 2 + embedding paragraph; add the
   safety-rules compatibility note to `connect.md` and `llms.txt`.
2. `lloom`: `lloom setup` umbrella command + tests (Option B).
3. `lloom`: credentials-file split or `--keep-password`, plus `lloom status`
   (Option D).
4. `lloom_chat` (optional, roadmap): enrollment-token registration (Option C).
