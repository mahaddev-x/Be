# Contributing to BeeHive

Thanks for your interest in contributing. BeeHive is in public beta (v0.5.0) and actively looking for collaborators.

## Ways to contribute

- **Bug reports** — open a [GitHub Issue](https://github.com/mahaddev-x/beehive/issues/new?template=bug_report.md)
- **Feature requests** — open a [GitHub Discussion](https://github.com/mahaddev-x/beehive/discussions/new?category=ideas)
- **Pull requests** — see workflow below
- **New Bees** — share useful `.yaml` bee definitions in [Show & Tell](https://github.com/mahaddev-x/beehive/discussions/categories/show-and-tell)

## Development setup

Requirements: Node.js 20+, npm

```bash
git clone https://github.com/mahaddev-x/beehive.git
cd beehive
npm install
npm link             # registers `beehive` command globally from source
beehive --version    # should print 0.5.0
```

Changes to `src/` are picked up on the next `beehive` invocation — no rebuild needed (tsx runs TypeScript directly).

Run `npm run build` to compile to `dist/` for a production test.

## PR workflow

1. Fork the repo
2. Create a branch: `git checkout -b feat/my-feature` or `fix/my-bug`
3. Make your changes
4. Test: `beehive bee test sentiment-scorer --input '{"text":"hello"}'`
5. Open a PR against `main`

## Commit style

```
feat: add support for TOML input files
fix: handle 429 retry-after header correctly
docs: update provider list in README
chore: bump version to 0.5.1
```

## Code style

- TypeScript strict mode, no `any` unless unavoidable
- No new dependencies without discussion — we keep the dependency list lean
- If it's provided by pi-agent (`pi-ai`, `pi-agent-core`, `pi-tui`), use it instead of reimplementing

## Questions?

Open a [Discussion](https://github.com/mahaddev-x/beehive/discussions) — happy to help.
