# Charter — Agents can drive the Review tab

<!-- Written by /gsd-path-define program mode. Enduring program intent:
     vetoes and constraints here override every milestone. Amended only by an
     explicit user ruling recorded under Corrections. Lives at .project/
     top level and persists across milestones; it never archives. -->

Review panel: off

Source spec: `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\SPEC.md`
(frozen copy of the owner's define reply of 2026-11-18, sha256 `4eefb853…e88d0`).
Lines marked `Delegated (driver D0NN)` were answered for the owner by the
gsd-path driver and can be overruled; the log is
`C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\DECISIONS.md`.

Naming: "the Charter" inside the spec means the product's own decision
records, read here as ADR 0014 (identity from the person, scope from the
token) and ADR 0015 (a recorded sentence waits for a nod). It is not this
file. Stated for correction.

## Vision

The Review tab is the only way to maintain a Utopia knowledge graph, and it
works only from a browser session. On a large machine-extracted base the
clean-up cannot be done one click at a time, and the agents that can already
read the graph and see the problems cannot act on them. When the program
ends, an agent holding a write-scope personal token owned by an editor can
work every section of the Review tab over MCP and the token-authenticated
API, under an explicit rule for what it may finalize and what it may only
propose, with every action attributed and every merge revertible.

## Full scope: in

- Token access to Review reads: each of the thirteen Review sections can be
  listed, and an item's evidence read, with a personal token: Overview,
  Awaiting your nod, Duplicates, Conflicts, Unconfirmed, Low confidence,
  Axioms, Ontology, Alignment, Errata, Agent History, Decisions, Merges.
- Token access to Review writes: confirm, reject, merge, revert, resolve and
  batch, for a write-scope token whose owner is an editor of the base.
- MCP tools for the same reads and writes, beside the existing eleven read
  tools and `remember`.
- Agent-authority policy: a per-operation rule for which operations an agent
  finalizes and which it only proposes, ruled by the owner and enforced by
  the server.
- Proposal path: operations ruled propose-only wait for a person's nod before
  they change the graph, as `remember` does.
- Attribution: every agent action appears in Activity and Agent History under
  the token owner and the agent's name.
- Merge revert: every merge an agent makes can be reverted.
- Decision record and documentation for the newly opened routes and tools,
  as upstream conventions require (see Constraints).

## Full scope: out (vetoes)

- Straight-through agent writes built before, or outside, the owner's
  finalize-versus-propose ruling — "Without it the factory is likely to build
  straight-through agent writes and remove the safeguard the product was
  designed around." (spec, Hard constraint)
- Weakening the existing nod gate on `remember` — "The Charter makes remember
  wait for a person's nod because a document can talk an agent into recording
  something; the same risk applies to an agent confirming or merging facts on
  the strength of text it just read." (spec, Goal)
- Widening a token beyond its owner or its scope: a read-scope token, or a
  token whose owner is below editor, gains no Review write — "an agent
  holding a write-scope token owned by an editor" (spec, Goal); ADR 0014
  decision 1.

## Constraints

- Stack is the existing one: Rust workspace (axum 0.8, sqlx, Postgres 16 with
  pgvector), React 19 / Vite web app. Source: `research/evidence-codebase.md`.
- Ground truth at `3d307f6` (checked in code, corrects two spec facts):
  - Every handler in `crates/utopia-server/src/api/review_routes.rs` takes
    `AuthUser` (session only) except `decide_alignment_kind_word`, which
    takes `SessionOrToken`. The spec's Review-route fact holds.
  - Personal tokens already read `/kbs/{id}/entities`, `/ontology`,
    `/export`, graph and search routes (ADR 0014 decision 7). The spec's
    "401 even on /export" does not hold for this checkout; the program builds
    on these reads rather than adding them.
  - Tokens act as their owner, scope only narrows, every call re-checks scope
    and writes an audit row (ADR 0014 decisions 1, 2, 5). No base-level
    machine token or service account (ADR 0014, Dead ends).
- Delegated (driver D002): the work runs as a program with a roadmap — the
  contract's recommended shape; no scope is dropped and nothing is built
  before charter and roadmap approval.
- Delegated (driver D003): the finalize-versus-propose decision is researched
  first and then ruled by the owner at the decide gate — spec, Goal: "The
  milestone must also decide which operations an agent may finalize and which
  it may only propose."
- Delegated (driver D005): fork first, upstream later. Work lands on
  `evolv3ai/utopia` `dev` (integration `direct`, as STATE records). Upstream
  conventions are binding so the work can be proposed to `deeplethe/utopia`
  later without rework: an ADR before a public-API or data-model change, DCO
  `Signed-off-by`, Chinese explanatory comments, sentence-named tests and
  migrations, one behaviour-sentence commit subject per change. Nothing is
  sent to `deeplethe/utopia` by this program. The contract gave no
  recommendation here; this option forecloses neither alternative.
- Delegated (driver D006): store and server behaviour is verified against a
  migrated local Postgres 16 + pgvector with `UTOPIA_TEST_REQUIRE_DB=1`. A
  plain `cargo test` without a database is not evidence (codebase map).
- Delegated (driver D007): this machine's toolchain (Windows, Node 26, pnpm
  10.14) is acceptable Verify evidence; the difference from CI is a risk.
- Delegated (driver D008): review panel `off`, the configured preference.
- Existing session-cookie behaviour of the Review tab and its routes keeps
  working unchanged (brownfield protection; stated for correction).
- No budget, deadline, paid service, or deployment is in scope. The owner's
  vibecoding.bot instance is not touched by the program.
- Doc-versus-code rulings from the inspect audit (full rows in
  `research/DOCS-AUDIT.md` `## User rulings`); all are queued, none planned:
  - Queue 1, 11 — Delegated (driver D009, D012): fix-doc.
  - Queue 4 — Delegated (driver D010): fix-doc.
  - Queue 6 — Delegated (driver D011): fix-doc, no toolchain pin.
  - Queue 13, 14 — Delegated (driver D013, D014): accept-drift.
  - Queue 16 — Delegated (driver D015): fix-doc, no stdout feature.
  - Queue 17 — Delegated (driver D016): fix-doc.
  - Queue 20 — Delegated (driver D017): fix-doc.
  - Queue 28 — Delegated (driver D018): fix-doc.

## Program success criteria

1. With a personal token, each of the thirteen Review sections can be listed
   over the token API and over MCP, and returns the same items a signed-in
   editor sees in that section of the Review tab.
2. For any listed item, the token API and MCP return its evidence (the
   sentences, facts, or entities the Review tab shows for it).
3. With a write-scope token owned by an editor, an agent can confirm, reject,
   merge, revert, resolve, and batch over the token API and over MCP; each
   call either changes the graph or lands as a proposal, exactly as the
   owner's per-operation ruling says.
4. An operation ruled propose-only does not change the graph, retrieval, or
   reasoning until a person nods, and appears in a Review queue for that nod.
5. A read-scope token, and a write-scope token whose owner is below editor,
   are refused on every Review write with a stable error code; a token whose
   `kb_ids` exclude the base cannot see it.
6. Every agent action appears in Activity and in Agent History, naming the
   token owner and the agent.
7. Every merge made by an agent can be reverted, by the agent over the token
   API or MCP and by a person in the Review tab, and the graph returns to its
   state before the merge.
8. The existing Review tab works as before for a signed-in person, and
   `remember` still waits for a nod.

## Open questions

- [RESEARCH] Which operations may an agent finalize and which may it only propose? Gather the precedents (ADR 0015 nod, ADR 0025 governor, ADR 0027 merge gate, ADR 0043 governed queues, ADR 0044 errata gate) and each operation's reversibility, including what a wrong action sends out of the graph. Decide proposes a per-operation table; the owner rules on it (Delegated, driver D003). This ruling is a `NEEDS-USER` item at decide.
- [RESEARCH] How is "the agent's name" known to the server: the token's name, MCP `clientInfo`, or a declared field? What do Activity and Agent History show today for the governor and errata agents?
- [RESEARCH] Map each of the thirteen Review sections to its existing routes and store functions; name the sections with no dedicated read route.
- [RESEARCH] What do `review/batch` and `review/agent/{decision_id}` do today, and what does a batch of agent actions do to the confused-deputy risk?
- [RESEARCH] Do token read routes behave on a running build of this checkout as ADR 0014 decision 7 says? The owner's deployed build is unknown (Delegated, driver D004: "Not sure"); the program verifies its own build.
- [RESEARCH] Which local Postgres 16 + pgvector instance can Verify use on this machine (the compose service on 127.0.0.1:1517, or another)?
- [RESEARCH] Are migration numbers 0081–0089, 0107, 0108, 0112, 0124 and ADR numbers 0055–0059, 0068 reserved upstream, and how should new numbers be chosen to avoid colliding with `deeplethe/utopia`?
- [RESEARCH] The spec reports every fact at 100% confidence on the owner's base. Does that leave the Low confidence section empty for agents, and is it extraction behaviour? The program does not scope a fix.
- [RESEARCH] Largest assumption, challenged: that an agent reading untrusted base content can be allowed to finalize anything safely. If it cannot, the program still delivers reads plus proposals for every operation; research should say what that leaves a person to click on a large base.

## Corrections

<!-- Verbatim user rulings that amend this charter. Append-only. -->
