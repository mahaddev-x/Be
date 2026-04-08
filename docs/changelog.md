# Changelog

All notable changes to BeeHive are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.5.0] - 2026-04-09 (Beta)

This is the first public beta. The architecture is stable and all core features
work. We're opening it up for collaborators and early testers before a 1.0 release.

### Added
- Complete rewrite in TypeScript, built on **pi-mono** by Mario Zechner
  - Every Bee is a full `pi-agent-core` Agent with multi-turn conversations,
    tool calling, event-driven streaming, and automatic retry handling
  - 20+ LLM providers via `@mariozechner/pi-ai` unified routing layer
  - Live streaming output via `@mariozechner/pi-tui` differential renderer
- **5 built-in Bees**: `sentiment-scorer`, `text-classifier`, `data-extractor`,
  `url-scraper`, `file-reviewer`
- **Built-in tools**:
  - `fetch_url` — Readability + Turndown pipeline (same as Claude Code / Codex CLI)
  - `read_file` — reads any local file
- **Input formats**: CSV, JSON array, JSONL
- **Output formats**: table, JSON, CSV, JSONL
- Job persistence to `~/.beehive/jobs/` as JSONL with full token/cost tracking
- Parallel swarm execution with configurable concurrency (`--parallel`)
- JSON Schema output validation with soft warnings on missing fields
- Retry logic with exponential backoff and `Retry-After` header support
- Interactive setup wizard (`beehive setup`) for all LLM providers
- `beehive bee test` with live streaming output (pi-tui on TTY, plain on pipe)
- Custom tool plugins via `plugins:` in bee YAML
- **Cross-platform native binary builds** via `bun build --compile`
  - Linux x64, Linux ARM64, macOS Intel, macOS Apple Silicon, Windows x64
- **Fully automated release pipeline** via GitHub Actions
  - Tag push → build all 5 binaries → GitHub Release → npm publish
- **Zero execution-policy issues on Windows** — `.ps1` removed, `.cmd` only

### Architecture
- `beehive run` dispatches N pi-agents in parallel (p-limit)
- Each agent has its own conversation state, tool access, and retry budget
- Results streamed to disk as they complete — no memory accumulation on large jobs
- `bin/beehive.js` → tsx hot-reload in dev; `dist/index.js` → compiled for npm

### Known limitations (to fix before 1.0)
- No MCP (Model Context Protocol) server support yet
- No streaming results to stdout during batch runs
- Windows ARM64 and Linux x86 not supported (Bun limitation)
- No web UI or dashboard
