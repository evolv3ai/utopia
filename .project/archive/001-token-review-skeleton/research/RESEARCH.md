# Research Handoff

Phase: research
Status: complete
Intent: `.project/CHARTER.md`

## Dispatch

- `domain` — dispatched → `.project/research/evidence-domain.md` — the agent-authority policy needs the product's own precedents and outside practice on agent writes over untrusted content
- `stack` — dispatched → `.project/research/evidence-stack.md` — token read behaviour of this build, the local Postgres for Verify, and upstream number reservations are open
- `pitfalls` — dispatched → `.project/research/evidence-pitfalls.md` — confused-deputy, batch, and confidence failure modes bear on which operations may finalize
- `similar` — dispatched → `.project/research/evidence-similar.md` — comparable products that let agents or bots curate a shared graph inform the finalize-versus-propose table
- `capability-map` — dispatched → `.project/research/evidence-capability-map.md` — program scope: what exists per Review section versus what must be built

## Question assignments

- `` [RESEARCH] Which operations may an agent finalize and which may it only propose? Gather the precedents (ADR 0015 nod, ADR 0025 governor, ADR 0027 merge gate, ADR 0043 governed queues, ADR 0044 errata gate) and each operation's reversibility, including what a wrong action sends out of the graph. Decide proposes a per-operation table; the owner rules on it (Delegated, driver D003). This ruling is a `NEEDS-USER` item at decide. `` → `domain`, `similar`, `pitfalls`
- `` [RESEARCH] How is "the agent's name" known to the server: the token's name, MCP `clientInfo`, or a declared field? What do Activity and Agent History show today for the governor and errata agents? `` → `capability-map`
- `` [RESEARCH] Map each of the thirteen Review sections to its existing routes and store functions; name the sections with no dedicated read route. `` → `capability-map`
- `` [RESEARCH] What do `review/batch` and `review/agent/{decision_id}` do today, and what does a batch of agent actions do to the confused-deputy risk? `` → `capability-map`, `pitfalls`
- `` [RESEARCH] Do token read routes behave on a running build of this checkout as ADR 0014 decision 7 says? The owner's deployed build is unknown (Delegated, driver D004: "Not sure"); the program verifies its own build. `` → `stack`
- `` [RESEARCH] Which local Postgres 16 + pgvector instance can Verify use on this machine (the compose service on 127.0.0.1:1517, or another)? `` → `stack`
- `` [RESEARCH] Are migration numbers 0081–0089, 0107, 0108, 0112, 0124 and ADR numbers 0055–0059, 0068 reserved upstream, and how should new numbers be chosen to avoid colliding with `deeplethe/utopia`? `` → `stack`
- `` [RESEARCH] The spec reports every fact at 100% confidence on the owner's base. Does that leave the Low confidence section empty for agents, and is it extraction behaviour? The program does not scope a fix. `` → `pitfalls`
- `` [RESEARCH] Largest assumption, challenged: that an agent reading untrusted base content can be allowed to finalize anything safely. If it cannot, the program still delivers reads plus proposals for every operation; research should say what that leaves a person to click on a large base. `` → `domain`, `pitfalls`
