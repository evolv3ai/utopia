# Patch Findings

<!-- Written by final review when it blocks and consumed by patch planning. -->

Reviewed HEAD: 8d4bfcf4efc032716b196a570b5dbb5b66c4a0dc
State: ship/blocked

## Findings

### P001 — Activity rows for a person's answer to a token proposal do not name the token

- **Source**: `.project/review/FINAL.md`
- **Locator**: SC7
- **Evidence**: In Activity, the rows for a person's decline (`agent.proposal.declined`), supersession (`agent.proposal.superseded`) and approval (the person's decision row carrying `agent_proposal`) of a token proposal do not read "owner via token name": they show "Mira Okafor declined an agent proposal", "Mira Okafor closed an agent proposal that no longer applies" and e.g. "Mira Okafor violation.decided", although E20 requires an Activity row reading owner via token name for "a proposal, its approval, decline or supersession". The token name is stored (`detail.proposed_by_token` on decline and supersession) but never rendered; approval rows carry only `agent_proposal`.
- **Fix direction**: Keep the person as the actor (N2, N10) and add the proposing token to the row: render decline and supersession as e.g. "Mira Okafor declined a proposal of Mira Okafor via graph-curator" from `detail.proposed_by_token` (or have `/audit` return it as a separate field, such as `proposed_by_token`, so `KbSettings.tsx` can render it), and do the same for an approval row that carries `agent_proposal` (store `proposed_by_token` in that audit detail when the approval runs). Change `KbActivity.test.ts:220-237`, which today asserts the token is absent, and make the E20 test assert the rendered text.
