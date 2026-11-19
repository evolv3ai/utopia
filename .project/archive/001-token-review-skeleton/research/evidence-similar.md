# Evidence — similar

<!-- Written by one researcher role. Consumed by the decider. -->

Dimension: similar
Questions assigned: Which operations may an agent finalize and which may it only propose? Gather the precedents (ADR 0015 nod, ADR 0025 governor, ADR 0027 merge gate, ADR 0043 governed queues, ADR 0044 errata gate) and each operation's reversibility, including what a wrong action sends out of the graph. Decide proposes a per-operation table; the owner rules on it (Delegated, driver D003). This ruling is a `NEEDS-USER` item at decide.

Web capability: confirmed (WebSearch and WebFetch, 2026-11-18). All sources below were opened on that date. Pages were read through a fetch-and-summarise tool; quotations are as that tool returned them.

## Finding: Wikidata lets an approved bot finalize edits, but gates the task, not each edit

- **Claim**: Wikidata policy lets a bot write directly to the shared graph once its task is approved: a request at Requests for permissions/Bot, a test run "of between 50 and 250 edits", then a bureaucrat grants the bot flag. The bot may not be used "for contributions that do not fall within the scope of the bot's designated tasks". No per-edit human approval exists.
- **Source**: https://www.wikidata.org/wiki/Wikidata:Bots (second half of the page, offset 100000)
- **Confidence**: high
- **Why it matters here**: The precedent for "agent finalizes" is a per-task grant with a bounded trial, which maps to a per-operation ruling (confirm, reject, merge, revert, resolve, batch) rather than one switch for all Review writes.

## Finding: Wikidata and Wikipedia attribute a bot to a named account and hold the operator responsible for repair

- **Claim**: Wikidata: "The contributions of a bot account remain the responsibility of its operator, who should be identified on its user page" and "The bot operator is responsible for cleaning up any damage caused by the bot"; bots "should always use descriptive edit summaries". English Wikipedia: operators "create a separate account in order to operate a bot", and "the bot operator is responsible for the repair of any damage caused by a bot which operates incorrectly".
- **Source**: https://www.wikidata.org/wiki/Wikidata:Bots ; https://en.wikipedia.org/wiki/Wikipedia:Bot_policy (first 100000 characters read; the last 16770 were not)
- **Confidence**: high
- **Why it matters here**: Supports charter criterion 6 (every action names the token owner and the agent): the two-name model (operator plus bot) is the established shape, and it differs from ADR 0014 only in that this product has no separate bot account, so the agent name must be carried beside the owner.

## Finding: Wikipedia keeps context-sensitive judgement away from unsupervised bots

- **Claim**: English Wikipedia's bot policy says unsupervised bots should avoid context-sensitive changes that normally need human judgement unless the operator shows no false positives or there is consensus for unsupervised runs; it separates assisted editing (tools that "do not alter Wikipedia's content without some human interaction") from fully automated bots; and "Approval of a bot for mass creation does not override the need for community consensus for the creation itself".
- **Source**: https://en.wikipedia.org/wiki/Wikipedia:Bot_policy
- **Confidence**: medium
- **Why it matters here**: Confirm, merge, and resolve are judgement calls on text the agent just read; this precedent places them on the "assisted" (propose) side unless a measured false-positive rate justifies otherwise. Batch is the "mass" case and needs its own ruling, not inheritance from the single-item ruling.
- **Limits**: The rule text was paraphrased by the fetch tool except the quoted fragments.

## Finding: A fully autonomous bot that finalizes is allowed where its action is itself an undo and its error rate is capped

- **Claim**: ClueBot NG reverts vandalism "quickly and automatically" with no per-edit approval. Its threshold is set to "hold false positives at a maximal rate of 0.1%" (catching about 40% of vandalism at that setting); "The same user/page combination is not reverted more than once per day"; a wrongly reverted editor is told to "simply redo your edit"; and "Administrators may turn the bot off by changing this page to 'False'".
- **Source**: https://en.wikipedia.org/wiki/User:ClueBot_NG
- **Confidence**: high
- **Why it matters here**: The one widely accepted case of unreviewed bot finalization has four properties: the action restores a prior state, a person can redo it in one step, the bot does not repeat the action against the same target, and there is a kill switch. In this product that profile fits revert (of a merge) and possibly reject, not confirm or merge.

## Finding: Wikipedia's pending-changes model holds an edit out of the reader's view until a reviewer accepts

- **Claim**: On pages under pending changes, edits by unregistered or new users are saved but marked pending; logged-out readers see "the most recent" accepted revision while logged-in users see the latest pending change; a change takes effect for readers when "reviewed and accepted by a pending changes reviewer" or an administrator.
- **Source**: https://en.wikipedia.org/wiki/Wikipedia:Pending_changes
- **Confidence**: high
- **Why it matters here**: Same shape as ADR 0015's nod and charter criterion 4: a proposal is stored and visible to reviewers but takes no part in what is served. It is a precedent for the proposal path, not for which operations go on it.
- **Limits**: The confidence level applies to the mechanism; the page says nothing about how bot edits are treated.

## Finding: Wikidata merge is undone by hand in two ordered steps, and other bots act on a merge before it is undone

- **Claim**: Undoing a Wikidata merge means restoring the pre-merge revision of the target item, then of the redirected item, in that order (the order is "important (especially if the item has sitelinks)"). The help page warns to "be absolutely certain" before merging and notes that some bots "are known to replace the redirected items in statements with their target redirects", so a wrong merge propagates into other items' statements and those edits must be reverted separately (KrBot's "may be reverted en masse" through EditGroups).
- **Source**: https://www.wikidata.org/wiki/Help:Merge
- **Confidence**: high
- **Why it matters here**: Independent confirmation of ADR 0027's point: reverting a merge restores the two items but not what read the merged state in between. It supports treating merge as the least safely finalizable operation and keeping the ADR 0027 impact gate in front of any agent merge.

## Finding: Batches on Wikidata are undoable as a unit, by anyone, with partial failure

- **Claim**: EditGroups tracks batches from QuickStatements, OpenRefine and other tools; undoing a batch "creates itself a new edit group that attempts to revert each edit in the group"; reverting can partly fail when later edits touched the same data; any user can undo another user's batch; undoing a batch that created items needs an administrator. The tool is "an unofficial tool maintained by volunteers" because MediaWiki has no built-in notion of an edit group.
- **Source**: https://www.wikidata.org/wiki/Wikidata:Edit_groups
- **Confidence**: high
- **Why it matters here**: For the batch operation: comparable systems that allow finalized batches give the batch an identity so it can be listed and undone as one unit, and they accept that undo is best-effort. A batch of agent actions without a batch id has no such undo; a batch of irreversible actions (confirm, reject, resolve) has none either way.

## Finding: Current Wikidata bot requests put merges out of scope by their own declaration

- **Claim**: Of the four bot requests visible in the first 100000 characters of the requests page on 2026-11-18, two exclude merging in their own safety rules: MecateBot ("Out of scope in all stages: merges, deletions, replacing others' statements") and UT-Bot ("no removal or replacement of existing sitelinks or labels, no merges"). No visible request was for an auto-merging or LLM-driven bot.
- **Source**: https://www.wikidata.org/wiki/Wikidata:Requests_for_permissions/Bot (first 100000 of 529325 characters read)
- **Confidence**: low
- **Why it matters here**: Weak but consistent signal that operators seeking approval treat merge as the operation to leave to people.
- **Limits**: Four of roughly seventy-three requests were seen; outcomes were not shown.

## Finding: MDM products split merge by rule class: automatic rules finalize, the rest queue for a steward

- **Claim**: Informatica MDM SaaS: "Based on the merge configuration of the model, the process merges the records automatically, queues the records for manual merge, or skips the records as not a candidate for merge." Reltio: "If the process determines that the records are a perfect match, they're automatically merged"; otherwise "A Data Steward is given the task to review these matches manually, and then decide if they are a match or not a match."
- **Source**: https://onlinehelp.informatica.com/IICS/dev/b360/en/hh-b360-define-jobs/Defining_match_and_merge_jobs.html ; https://docs.reltio.com/en/objectives/resolve-potential-matches/potential-matching-at-a-glance/potential-matching-operation/review-potential-matches-overview
- **Confidence**: high
- **Why it matters here**: The industry split for merge is not "machine versus person" but "which pairs": a deterministic class finalizes, everything else is a proposal. This is the same structure as ADR 0025 (confidence bar) plus ADR 0027 (hold), so an agent merge can reuse the existing gate rather than receive a blanket ruling.
- **Limits**: The confidence level applies to the three-way split; neither page states thresholds or the reviewer's role.

## Finding: MDM unmerge is a first-class operation, and automatic unmerge can fight a manual merge

- **Claim**: Reltio documents unmerge as both manual and automatic ("either manually unmerge these records, or enable the new automatic unmerge feature in your tenant"); automatic unmerge must be enabled per tenant. A 2025 Reltio community thread (surfaced by search, not opened) reports records merged by hand being immediately auto-unmerged when auto-unmerge was configured to cover manual merges.
- **Source**: https://docs.reltio.com/en/objectives/resolve-potential-matches/potential-matching-at-a-glance/potential-matching-operation/unmerge-entity-records (opened); community thread https://community.reltio.com/discussion/issue-automatic-unmerge-after-manual-merge-of-new-entities (search result only)
- **Confidence**: low
- **Why it matters here**: Charter criterion 7 lets an agent revert merges. If an agent can both merge and revert, and a person can too, the lead is a ping-pong risk between automated and human decisions on the same pair; ClueBot NG's once-per-target rule is the comparable mitigation.
- **Limits**: The finding is held at the lower of its two levels: medium for the documented unmerge feature (page opened), low for the conflict report (community thread seen as a search result only).

## Finding: Tamr keeps model suggestions on verified records as suggestions

- **Claim**: In Tamr Core a curator's "Verify and enable suggestions" keeps the model suggesting, but "any new suggestions are not applied automatically"; only "Verify and auto-accept suggestions" lets the system move records later (Tamr "can still move the records around if it disagrees with you"); "Verify and disable suggestions" stops suggestions for those records.
- **Source**: https://docs.tamr.com/new/docs/curating-clusters
- **Confidence**: high
- **Why it matters here**: A person's decision outranks later machine decisions by default, and letting the machine override it is an explicit per-record opt-in. For this product: an agent should not finalize against an item a person has already decided (a reverted merge, a rejected fact, a human keep) without that being a separate, stated permission.

## Finding: Senzing offers no curated merge; a forced link is data, and forcing apart is discouraged

- **Claim**: Senzing forces records together by giving them the same `TRUSTED_ID_TYPE` and `TRUSTED_ID_NUMBER` ("TRUSTED_ID will force together records that share the same TRUSTED_ID_TYPE and TRUSTED_ID_NUMBER attributes"), splits by giving different numbers ("Forcing records apart isn't a common task"), and advises checking for more attributes first so the engine decides on evidence. The article describes no undo other than changing the record's attributes again.
- **Source**: https://senzing.zendesk.com/hc/en-us/articles/360023523354
- **Confidence**: medium
- **Why it matters here**: An alternative design in which an override is an attributed, removable assertion rather than a destructive merge. It supports the product's existing choice (ADR 0019/0027: merge is undoable inside the graph) and shows that "keep apart" is treated as cheap and rare elsewhere too (ADR 0027 decision 2).
- **Limits**: The source is a support article; undo behaviour is not stated in it.

## Finding: Knowledge-graph memory MCP servers expose direct writes and bulk deletes with no review step, no undo, and weak attribution

- **Claim**: Graphiti's MCP server exposes `add_memory`, `add_triplet` (adds a fact directly, bypassing extraction), `delete_entity_edge`, `delete_episode` and `clear_graph`, and its README says the server has "no authentication, no authorization, and no prompt-injection protection". Mem0's MCP server exposes `add_memory`, `update_memory`, `delete_memory`, `delete_all_memories` and `delete_entities`, with `list_events` for inspecting operations and no documented rollback. Cognee's MCP server exposes `remember`, `recall` and `forget` (including `everything=true`), with no documented approval, undo, or authorship record. None documents a proposal queue.
- **Source**: https://github.com/getzep/graphiti/blob/main/mcp_server/README.md ; https://docs.mem0.ai/platform/mem0-mcp ; https://docs.cognee.ai/cognee-mcp/mcp-tools
- **Confidence**: medium
- **Why it matters here**: The closest product category gives no precedent for a finalize-versus-propose split: these are single-agent memories, not shared reviewed graphs. The charter's veto on straight-through agent writes is a deliberate departure from the category norm, and no reusable policy table exists to copy from it.
- **Limits**: The finding is held at the lower of its two levels: high for the tool lists, medium for "no review step", which is an absence in the documentation read, not a tested behaviour.

## Finding: The MCP specification puts confirmation on the client, as SHOULD, and tool annotations are hints

- **Claim**: MCP 2025-06-18 says "there **SHOULD** always be a human in the loop with the ability to deny tool invocations" and clients "**SHOULD**: Prompt for user confirmation on sensitive operations". `ToolAnnotations` carries `readOnlyHint` (default false), `destructiveHint` (default true; false means "only additive updates"), `idempotentHint` (default false) and `openWorldHint`; "all properties in ToolAnnotations are **hints**. They are not guaranteed to provide a faithful description of tool behavior", and clients "**MUST** consider tool annotations to be untrusted unless they come from trusted servers". Servers "**MUST**" validate inputs, "Implement proper access controls" and "Rate limit tool invocations".
- **Source**: https://modelcontextprotocol.io/specification/2025-06-18/server/tools ; https://modelcontextprotocol.io/specification/2025-06-18/schema
- **Confidence**: high
- **Why it matters here**: A server cannot count on the MCP client to ask the person. The charter's "enforced by the server" is the only enforcement the protocol guarantees; annotations are still worth setting (`destructiveHint: true` on merge, reject, resolve, batch; `idempotentHint` on confirm and revert) so that well-behaved clients prompt, but they are not the gate.
- **Limits**: The confidence level applies to the 2025-06-18 revision; whether a later revision changed this text was not checked.

## Finding: MCP elicitation is optional per client, so it cannot be the nod

- **Claim**: Elicitation lets a server ask the user through the client mid-call, with `accept`, `decline` and `cancel` responses. Clients that support it "**MUST** declare the `elicitation` capability"; the page notes it "is newly introduced in this version of the MCP specification and its design may evolve"; schemas are limited to flat primitive fields; and it is the client that presents the request to the user.
- **Source**: https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation
- **Confidence**: high
- **Why it matters here**: Elicitation could add an in-conversation confirmation for a finalize call, but a client that does not declare it, or an unattended agent, gives no person. The person who answers is whoever sits at the MCP client, not necessarily an editor in the Review tab. It supplements a server-side proposal queue; it does not replace one.

## Finding: MCP `clientInfo` is self-declared

- **Claim**: The `initialize` request carries `clientInfo` with `name`, `title` and `version` as "Client implementation information"; the lifecycle page describes no verification of these fields.
- **Source**: https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle
- **Confidence**: high
- **Why it matters here**: If "the agent's name" in Activity and Agent History comes from `clientInfo`, it is a label supplied by the caller, comparable to a bot's edit summary, and should be shown as declared beside the authenticated token owner (charter criterion 6).
- **Limits**: The confidence level is that no verification is described on that page; the authorization pages were not read.

## Finding: GitHub's model is "a bot may propose and may not approve its own proposal"

- **Claim**: "Pull request authors cannot approve their own pull requests." Branch protection can require that "the most recent reviewable push must be approved by someone other than the person who pushed it". For new personal repositories, "workflows are not allowed to create or approve pull requests" by default (`GITHUB_TOKEN`), with an explicit opt-in setting "Allow GitHub Actions to create and approve pull requests". Copilot's cloud agent "can only push to a single branch", and "Actions workflows triggered by pull requests raised by the agent require approval" from a user with write access.
- **Source**: https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/approving-a-pull-request-with-required-reviews ; https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches ; https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository ; https://docs.github.com/en/copilot/responsible-use/copilot-coding-agent
- **Confidence**: high
- **Why it matters here**: Two rules transfer directly. First, the proposer cannot be the approver: a proposal an agent makes with an editor's token should not be nodded through by a call on the same token (or by any token), only by a session. Second, the default is deny, and auto-approval is an explicit per-repository opt-in, which matches a per-base or per-operation owner ruling.
- **Limits**: The Copilot page as read does not state that the agent cannot merge its own pull request; that claim is not made here.

## Finding: GitHub attributes an agent's action to both the person and the app, and records the token type

- **Claim**: When a GitHub App acts on behalf of a user, "the GitHub UI will show the user's avatar photo along with the app's identicon badge"; audit logs "will list the user as the actor" with `programmatic_access_type` "GitHub App user-to-server token"; "The app can only access resources that the user has access to" and only what the app itself is permitted. Copilot's "commits are authored by Copilot, with the human who started the task marked as the co-author".
- **Source**: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-with-a-github-app-on-behalf-of-a-user ; https://docs.github.com/en/copilot/responsible-use/copilot-coding-agent
- **Confidence**: high
- **Why it matters here**: Matches ADR 0014 (identity from the person, scope from the token) and gives a concrete display for criterion 6: person as actor, agent as a badge beside them, and the access type stored in the audit row so agent actions can be filtered apart from the same person's clicks.

## Finding: On GitHub an undo is a new proposal, and conditional auto-merge is scoped to a low-risk class

- **Claim**: "Reverting a merged pull request creates a new pull request that reverts the original merge commit", which still has to be merged and needs write permission. GitHub's Dependabot automation guide gives an example that "automerges all patch updates to `my-dependency`" (condition: `version-update:semver-patch`) and advises enabling "Require status checks to pass before merging".
- **Source**: https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/reverting-a-pull-request ; https://docs.github.com/en/code-security/dependabot/working-with-dependabot/automating-dependabot-with-github-actions
- **Confidence**: high
- **Why it matters here**: Two options for the table. Revert can itself be gated (a proposal) or direct; GitHub gates it, Wikidata and ClueBot NG do not. And where a bot finalizes, the grant is narrowed to a named low-risk class with a machine check in front, the analogue of "agent may finalize a merge only when the ADR 0027 gate reports no impact".

## Finding: Per-operation precedents across comparable systems

- **Claim**: Across the systems opened for this dimension, no source prescribes a finalize-versus-propose table, but each Review operation has an outside precedent: merge is held for a person outside a machine-gated class, revert of a merge is the operation most often finalized by automation, confirm and resolve have precedent only as a reviewer's act, reject has no separate precedent from accept, and batch is finalized only where the batch has an identity and a unit undo. The per-operation bullets below carry the detail.
- **Source**: https://www.wikidata.org/wiki/Wikidata:Bots ; https://www.wikidata.org/wiki/Help:Merge ; https://www.wikidata.org/wiki/Wikidata:Edit_groups ; https://en.wikipedia.org/wiki/Wikipedia:Bot_policy ; https://en.wikipedia.org/wiki/Wikipedia:Pending_changes ; https://en.wikipedia.org/wiki/User:ClueBot_NG ; https://onlinehelp.informatica.com/IICS/dev/b360/en/hh-b360-define-jobs/Defining_match_and_merge_jobs.html ; https://docs.reltio.com/en/objectives/resolve-potential-matches/potential-matching-at-a-glance/potential-matching-operation/review-potential-matches-overview ; https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/approving-a-pull-request-with-required-reviews ; https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/reverting-a-pull-request ; https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- **Confidence**: medium
- **Why it matters here**: This is the outside-precedent input to the per-operation table the decider proposes for the owner's ruling (charter criteria 3 and 4); it is evidence per operation, not a ruling.
- **Limits**: This finding is a synthesis of the findings above, so it carries their limits. The in-repo precedents (ADR 0025, 0043, 0044) and each operation's reversibility in this codebase belong to the `domain` and `pitfalls` dimensions and were not established here.
- **merge** — Most often held for a person outside a deterministic class. MDM products auto-merge only a configured rule class and queue the rest (Informatica MDM SaaS job help; Reltio potential-matches overview). Wikidata's merge undo is manual and other bots propagate a merge before it is undone (Help:Merge). Lesson: propose by default; finalize only behind a machine gate such as ADR 0027's.
- **revert (of a merge)** — The operation most often finalized by automation, because it restores a prior state and is itself redoable (ClueBot NG; EditGroups batch undo by any user). GitHub is the counter-example: a revert is a new pull request that must be merged (GitHub reverting docs). Reltio shows the hazard of automated unmerge undoing a person's merge. Lesson: finalize is defensible, with a once-per-target limit or a rule against reverting a person's decision.
- **confirm** — Outside precedent treats acceptance of a pending change as the reviewer's act: pending-changes reviewers on Wikipedia, "Pull request authors cannot approve their own pull requests" on GitHub, and `GITHUB_TOKEN` approval off by default. Lesson: an agent confirming facts it or another machine proposed collapses proposer and approver; propose-only is the matching precedent.
- **reject** — No source separates rejecting from accepting. ClueBot NG is the nearest case of automated rejection being finalized, under a 0.1% false-positive cap and one-step redo. In this product a rejection also blocks re-proposal (ADR 0015 decision 5, `rejected_facts`), so the "one-step redo" property must be checked by `domain`/`pitfalls` before the analogy holds.
- **resolve (conflicts)** — No comparable system was found that lets a bot settle a contradiction between two sourced values without a person; Wikipedia's bot policy keeps context-sensitive judgement away from unsupervised bots (Wikipedia:Bot_policy). Lesson: propose-only is the only position with precedent.
- **batch** — Finalized batches exist where the batch has an identity and a unit undo (EditGroups) and the task was approved after a 50–250 edit trial (Wikidata:Bots). Wikipedia requires separate consensus for mass actions. Lesson: batch should not grant more than its members' rulings, and a finalizing batch needs a batch id and a size cap; otherwise propose.
- **Cross-cutting** — Server-side enforcement is required because MCP confirmation is a client SHOULD and annotations are untrusted hints (MCP 2025-06-18 tools and schema). Attribution should show person and agent together with the access type in the audit row (GitHub user-to-server tokens; Wikidata operator rule). A kill switch per agent or token is standard (Wikidata block; ClueBot NG run page).

## Assigned questions — answers

- Which operations may an agent finalize and which may it only propose? Gather the precedents (ADR 0015 nod, ADR 0025 governor, ADR 0027 merge gate, ADR 0043 governed queues, ADR 0044 errata gate) and each operation's reversibility, including what a wrong action sends out of the graph. Decide proposes a per-operation table; the owner rules on it (Delegated, driver D003). This ruling is a `NEEDS-USER` item at decide. → Comparable systems give evidence per operation, not a ruling, and no source prescribes a table for this product: outside precedent supports propose-only for confirm and resolve, propose by default for merge with finalize only behind a machine gate such as ADR 0027's, finalize as defensible for revert of a merge under a once-per-target limit, no separate precedent for reject, and batch granting no more than its members' rulings with a batch id and unit undo when it finalizes; server-side enforcement is required because MCP confirmation is a client-side SHOULD. Detail and sources are in "Finding: Per-operation precedents across comparable systems" and the findings it draws on (Wikidata: Bots, Help:Merge, Edit groups; Wikipedia: Bot policy, Pending changes, ClueBot NG; Informatica MDM SaaS and Reltio; GitHub; MCP specification 2025-06-18). The in-repo precedents (ADR 0025, 0043, 0044) and each operation's reversibility in this codebase were not established by this dimension; they belong to `domain` and `pitfalls`.

## Dead ends

- https://docs.informatica.com/master-data-management/multidomain-mdm/10-3/configuration-guide/.../consolidating_records_automatically_or_manually.html — HTTP 403 to the fetch tool; Informatica Multidomain MDM (on-premises) automerge/manual-merge text is known only from search snippets and is not cited as a finding. The MDM SaaS online help was used instead.
- Informatica unmerge behaviour — no primary page found; search returned Salesforce Trailhead (a different product), slide decks, and a third-party draft. Not used.
- Reltio "Not a Match" persistence (whether a rejected pair is suppressed from future suggestions) — the two Reltio pages opened do not say.
- Reltio unmerge mechanics (what is restored, crosswalk history) — the unmerge page is an overview and points to a tenant-configuration topic that was not opened.
- Senzing "how"/"why" audit of a forced resolution and a force-unresolve command — not found in the support article; the SDK reference was not opened.
- https://docs.mem0.ai/platform/features/mcp — 404; the working page is https://docs.mem0.ai/platform/mem0-mcp. Mem0 attribution fields (`user_id`, `agent_id`) are not described on that page.
- GitHub docs URLs under `/repositories/configuring-branches-and-rulesets/...` — 404 (three guessed paths); the correct path is under `/configuring-branches-and-merges-in-your-repository/managing-protected-branches/`.
- GitHub Copilot cloud agent "cannot approve or merge its own pull request" — not stated on the responsible-use page as read; left out rather than asserted.
- Wikipedia pending changes and bots — the policy page does not say how bot edits are treated under pending changes.
- Wikidata Requests for permissions/Bot — 529325 characters; only the first 100000 read, so no claim is made about whether any LLM-driven or merging bot has been approved or refused.
- MCP specification revisions after 2025-06-18 — not checked; findings are scoped to that revision.
- Zep (hosted product, as distinct from the open-source Graphiti MCP server) — not opened; no claim made about its hosted MCP tools.
