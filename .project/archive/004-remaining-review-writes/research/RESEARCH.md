# Research Handoff

<!-- Written by the research orchestrator and consumed by decide. -->

Phase: research
Status: complete
Intent: `.project/intent/INTENT.md`

## Dispatch

- `domain` — dispatched → `.project/research/evidence-domain.md` — the product's own decision records and the MCP specification settle what an answer to a governor decision is and how write tools may be named and grouped
- `stack` — dispatched → `.project/research/evidence-stack.md` — the ledger schema, the list and route shapes and the handler pattern decide how targets are named, where the new reads sit and where a proposal enters
- `pitfalls` — dispatched → `.project/research/evidence-pitfalls.md` — stale proposals, the nod gate and the governor's fuse are the failure modes the new proposal kinds can hit
- `similar` — skipped → none — the program research already compared outside products for the authority table; no open question of this milestone turns on a comparable product

## Question assignments

- `` [RESEARCH] The ruled table makes "answer a governor decision" propose only for a token, while the synthesis keeps the governor's own decision route session-only. Through which token route and MCP tool does an agent propose an answer to a governor decision, given that in code that answer is a second way to merge, keep, revert or manually merge a pair as a person (map F12)? The behaviour is ruled in edge E6; the route and tool are open. `` → `domain`, `pitfalls`
- `` [RESEARCH] How does the ledger name a target that has no single uuid (a kind word, an ontology proposal keyed by section and key, a run on the base), and does that need migration 0130 (map F4)? `` → `stack`
- `` [RESEARCH] Where do the token and MCP reads of mappings and open ontology proposals sit: a new section name on the existing list, or their own routes opened to a token (map F11, F12)? `` → `stack`
- `` [RESEARCH] How is "the target can no longer be decided" detected per operation without matching error texts, so that approval closes a stale proposal as `superseded` (map F5, F22)? `` → `pitfalls`
- `` [RESEARCH] Which MCP tool names and families satisfy "one write tool per operation family" under the name rules the M003 tests pin (map F20)? `` → `domain`
- `` [RESEARCH] Does a token's proposal enter through the person's route with a token branch (the nod route included) or through a separate route, given the tests that pin 401 on those routes and the charter veto on the nod (map F13, F19)? `` → `stack`, `pitfalls`
