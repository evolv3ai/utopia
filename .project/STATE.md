---
pipeline: gsd-path/v2
project: utopia
milestone: token-review-skeleton
phase: build
                    # inspect only for brownfield; greenfield starts at define
                    # roadmap only in program flow (CHARTER.md exists)
status: active
branch: gsd-path/M001
                    # the router rebinds before any next-milestone file change
                    # the bound branch is never main; ship integrates it there
archive: null       # persisted archive transaction path; never recomputed
integration_default: direct # direct | pull-request | external-landing; project setting
integration: direct # current milestone; may override the default before build
integration_source: default # default | milestone; preserves override provenance
default_branch: dev # origin default branch; initialization records it once, nothing changes it
---

# Project State

One file, always current. The router reads this first; every phase updates
it on completion. If this file and the artifacts disagree, the artifacts win
— fix this file.

## Log

<!-- append one line per transition: date, phase, event -->
- 2026-11-18 — inspect — project initialized
- 2026-11-18 — inspect — router bound initial milestone
- 2026-11-18 — inspect — inspection artifacts passed
- 2026-11-18 — define — definition started
- 2026-11-18 — define — pre-approval: {"kinds": ["intent", "plan"]}
- 2026-11-18 — define — program charter approved
- 2026-11-18 — research — research started
- 2026-11-18 — research — research gate passed: .project/research/RESEARCH.md; dispatched domain, stack, pitfalls, similar, capability-map; skipped none; 9 questions
- 2026-11-18 — decide — decision synthesis started
- 2026-11-18 — decide — synthesis validated: .project/SYNTHESIS.md
- 2026-11-18 — roadmap — roadmap started
- 2026-11-18 — roadmap — program roadmap approved
- 2026-11-18 — define — definition started
- 2026-11-18 — define — milestone intent approved
- 2026-11-18 — plan — planning started
- 2026-11-18 — plan — plan approved
- 2026-11-18 — build — build started
- 2026-11-19 — ship — build done; final review pending
- 2026-11-19 — ship — final review blocked: project Verify failed at c7db849 (utopia-cli CURRENT_SCHEMA_VERSION 113 against 114 applied migrations); evidence .project/review/final-gap-1.md
- 2026-11-19 — plan — patch plan reopened
- 2026-11-19 — plan — patch plan approved
- 2026-11-19 — build — build started
