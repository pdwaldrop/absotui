# AGENTS.md

Cross-tool agent instructions for this repository. Applies regardless of which
AI coding tool is being used (Claude Code, Sparky/Muse, or any future agent).

## Agent dialog protocol

This file doubles as a shared communication channel between agents working on
this repo. Use it to coordinate across sessions and tools: say what you did,
what you verified, what you decided, and what's still open — so the next agent
doesn't have to rediscover it.

1. **Append-only.** Add new entries at the end under `## Dialog`, newest last.
   Never rewrite or delete another agent's entry. Correct a mistake with a
   follow-up entry, not an edit.
2. **One entry per work session** (or per distinct task). Short and factual.
3. **Every entry has five parts:** date, agent, what changed, how it was
   verified, open questions / handoff notes.
4. **Decisions with lasting effect** go under `## Standing decisions`, dated,
   with who decided. Check that section before starting work.
5. **No secrets.** Never paste tokens, passwords, or API keys here. If a task
   involved credentials, say only that it did and where they're stored.
6. **Keep it human-readable.** Paul reads this too. Plain language, no
   internal tooling jargon.

## Standing decisions

- 2026-09-26 (Paul): No destructive actions without explicit authorization —
  never delete libraries/items, users, files, or data; never change server
  settings. Applies to every agent, every session.
- 2026-09-26 (Paul): Nothing is committed or pushed until the work has been
  shown running and Paul explicitly authorizes it.

## Dialog

### 2026-09-26 — Sparky (Muse)

**What changed:** Reviewed the Continue Listening and podcast Home list
pipelines and fixed a real desync bug: titles, progress, and durations were
built as separate parallel lists, so any entry missing media or metadata
shifted every list after it out of alignment (wrong progress under wrong
title). Same protection applied to podcast Home. Also added bounds protection
for podcast episode descriptions, removed per-row whole-list
formatting/cloning (each row now formats only its own values), replaced
repeated filter+contains list rebuilds with indexed gathers, and removed the
dead `format_sizes` helper (its tests now cover `format_size`).

**Files:** `src/api/utils/collect_personalized_view.rs`,
`src/api/utils/collect_personalized_view_pod.rs`, `src/app.rs`,
`src/ui/tui.rs`, `src/utils/format_size.rs`.

**How it was verified:** `cargo build` clean; `cargo test` 63 passed, 0 failed;
`cargo clippy --all-targets` shows only 8 pre-existing `too_many_arguments`
warnings in untouched code. Ran the real binary against Paul's live
Audiobookshelf server: login worked, library loaded (167 items), Continue
Listening showed 3 in-progress books with correct titles/progress/timing,
podcast Home listed 10 episodes, podcast search found and listed 62 episodes
of a show with no crashes or row misalignment. Note: the missing-metadata
edge case itself wasn't directly reproduced (no such entries existed), and
the test populated progress via API rather than in-TUI playback.

**Open questions / handoff:**
- Login only succeeds when the server address ends with `/` (without it, the
  request times out). Worth a UX fix or at least auto-appending the slash.
- Without `ABSOTUI_SECRET_KEY` set, token storage fails with a confusing
  login error. Should surface a clear message.
- `R` does a full reinit back to Library instead of preserving Home; `/`
  from the episode view jumps to global podcast search rather than searching
  episode text. Confirm whether these are intended.
- The repo is not rustfmt-formatted (`cargo fmt --check` flags nearly every
  file, including untouched ones). New code matches surrounding style instead.
  Reformatting the whole repo is a separate decision for Paul.
