---
pipeline: gsd-path/v2
project: utopia
milestone: gated-merge-proposals-revert-batch
phase: build
                    # inspect only for brownfield; greenfield starts at define
                    # roadmap only in program flow (CHARTER.md exists)
status: active
branch: gsd-path/M003
                    # the router rebinds before any next-milestone file change
                    # the bound branch is never main; ship integrates it there
archive: null
integration_default: direct
integration: direct
integration_source: default
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
- 2026-11-19 — ship — build done; final review pending
- 2026-11-19 — ship — final gate passed; shipping approval pending
- 2026-11-19 — shipped — archive preflight passed; shipment recorded
- 2026-11-19 — inspect — next milestone M002 token-review-reads bound on gsd-path/M002 at base 904b2a20380719553896160aa05d0af4f3a561ce; previous archive .project/archive/001-token-review-skeleton, ship 647c8678ae41dd0ff7e1ed98ddf3f351ad180902, integration 904b2a20380719553896160aa05d0af4f3a561ce, previous branch gsd-path/M001
- 2026-11-19 — inspect — inspection artifacts passed
- 2026-11-19 — define — definition started
- 2026-11-19 — define — milestone intent approved
- 2026-11-19 — plan — planning started
- 2026-11-19 — plan — plan approved
- 2026-11-19 — build — build started
- 2026-11-19 — build — plan defect repaired: T002 files gain review_totals_tests.rs for one transitional assertion (orchestrator answer, driver D091)
- 2026-11-19 — build — plan defect repaired: T002 files gain review_attribution_tests.rs, T002 supersedes T001 Verify and carries its checks (orchestrator answer, driver D092)
- 2026-11-19 — build — plan defect repaired: T003 Verify no longer freezes review_attribution_tests.rs, which T002 changed under driver D092
- 2026-11-19 — ship — build done; final review pending
- 2026-11-19 — ship — final gate passed; shipping approval pending
- 2026-11-19 — shipped — archive preflight passed; shipment recorded
- 2026-11-19 — inspect — next milestone bound: gsd-path/M003 at base 6c52ce8f8091b18d8713ce8d509fec756644a4f1; previous archive .project/archive/002-token-review-reads, ship dc385d45b5be5947a1a18171c79c9f34b8b2bc80, integration 6c52ce8f8091b18d8713ce8d509fec756644a4f1, branch gsd-path/M002
- 2026-11-19 — inspect — inspection artifacts passed
- 2026-11-19 — define — definition started
- 2026-11-19 — define — milestone intent approved
- 2026-11-19 — plan — planning started
- 2026-11-19 — plan — plan approved
- 2026-11-19 — build — build started
- 2026-11-19 — build — build blocked: T001 found edge E12 false for typed_fact_sources (merge copies statement links the unchanged revert never removes); ruling Delegated (driver D116) corrects the edge through build recovery
- 2026-11-19 — define — build intent corrections requested
- 2026-11-19 — define — build recovery: {"base": "f1cd6504cc583eccddeb7a72a4f809ff734ebbd1", "branch": "gsd-path/M003", "kind": "define", "source": "build"}
- 2026-11-19 — define — milestone intent approved
- 2026-11-19 — plan — planning started
- 2026-11-19 — plan — plan approved
- 2026-11-19 — build — build started
- 2026-11-19 — build — T004 brief repaired: two test helpers that build ReviewItem added to its files (driver D119)
- 2026-11-19 — build — wave 3 review passed; T005 Verify filter repaired to the mounted module path (wave 3 review observation)
