import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type {
  AgentDecision,
  AgentProposal,
  AlignmentItem,
  AxiomViolation,
  ConflictItem,
  ErrataItem,
  FactReviewItem,
  MergeLog,
  OntologyDefect,
  PendingFactItem,
} from "../api";
import { S } from "../i18n";
import { en } from "../i18n/en";
import { zh } from "../i18n/zh";
import { AgentProposalChip, AgentProposalChips, MergeActor } from "./AgentProposal";
import { PendingFactRow } from "./PendingFacts";
import {
  AgentRow,
  AlignmentPhraseRow,
  AlignmentRuleRow,
  ConflictRow,
  DefectRow,
  ErrataRow,
  FactRow,
  UnconfirmedRow,
  ViolationRow,
  agentRailCount,
} from "./Review";

const proposal: AgentProposal = {
  id: "019a0000-0000-7000-8000-000000000021",
  operation: "review.merge",
  reason: "kept_by_person",
  owner_name: "Mira Okafor",
  token_name: "graph-curator",
  created_at: "2026-11-19T09:30:00Z",
};

const chip = (p: AgentProposal) =>
  renderToStaticMarkup(createElement(AgentProposalChip, { proposal: p }));
const actor = (merge: Pick<MergeLog, "merged_by_name" | "agent">) =>
  renderToStaticMarkup(createElement(MergeActor, { merge }));

describe("the agent's open proposal on a card", () => {
  it("reads owner via token name and what the agent wants, with the reason in words", () => {
    const html = chip(proposal);
    expect(html).toContain("Mira Okafor via graph-curator");
    expect(html).toContain(S.review.tokenProposes["review.merge"]);
    expect(html).toContain(S.review.tokenReasons.kept_by_person);
    expect(html).not.toContain("review.merge");
    expect(html).not.toContain("kept_by_person");
  });

  it("has a label for each operation a proposal can carry", () => {
    for (const operation of ["review.merge", "merge.manual", "merge.revert"]) {
      const html = chip({ ...proposal, operation, reason: "merge_by_governor" });
      expect(html).toContain(S.review.tokenProposes[operation]);
      expect(html).toContain(S.review.tokenReasons.merge_by_governor);
      expect(html).not.toContain(operation);
    }
  });

  it("is a label, not a control, and uses the removed-user wording when the owner is gone", () => {
    const html = chip({ ...proposal, owner_name: null, reason: null });
    // 写成正则：风格守卫不许源码里出现原生控件的字面量
    expect(html).not.toMatch(/<(?:button|input|select|textarea)/);
    expect(html).toContain(`${S.kbset.deletedUser} via graph-curator`);
  });
});

describe("who made a merge, in the Merges section", () => {
  it("reads owner via token name for a merge a token agent made", () => {
    const html = actor({
      // 合并日志自己记的仍是令牌的主人：只看这一列就会写成主人亲手合的
      merged_by_name: "Mira Okafor",
      agent: {
        action_id: "019a0000-0000-7000-8000-000000000022",
        owner_name: "Mira Okafor",
        token_name: "graph-curator",
      },
    });
    expect(html).toBe(S.review.mergedByToken("Mira Okafor via graph-curator"));
    expect(html).toContain("Mira Okafor via graph-curator");
    expect(html).not.toBe(S.review.mergedBy("Mira Okafor"));
  });

  it("reads as today for a person's merge and for the adjudicator's", () => {
    expect(actor({ merged_by_name: "Mira Okafor" })).toBe(S.review.mergedBy("Mira Okafor"));
    expect(actor({ merged_by_name: "Mira Okafor" })).not.toContain(" via ");
    expect(actor({ merged_by_name: null })).toBe(S.review.mergedByAi);
  });

  it("uses the removed-user wording when the token's owner is gone", () => {
    expect(
      actor({
        merged_by_name: null,
        agent: {
          action_id: "019a0000-0000-7000-8000-000000000022",
          owner_name: null,
          token_name: "graph-curator",
        },
      }),
    ).toContain(`${S.kbset.deletedUser} via graph-curator`);
  });
});

/* ---------- Review 其余各档的卡片（0076） ---------- */

const ID = "019a0000-0000-7000-8000-0000000000";
const noop = () => {};

/** 一条只能提议的提议：有动作，理由是 `propose_only` */
const proposed = (n: number, operation: string, action: string): AgentProposal => ({
  id: `${ID}${n}`,
  operation,
  reason: "propose_only",
  owner_name: "Mira Okafor",
  token_name: "graph-curator",
  created_at: "2026-11-20T09:30:00Z",
  action,
});

const chips = (proposals?: AgentProposal[] | null) =>
  renderToStaticMarkup(createElement(AgentProposalChips, { proposals }));

/** 卡片上那一条提议该读成什么：主人 via 令牌名、操作的说法、动作的说法 */
function ensureReads(html: string, p: AgentProposal) {
  expect(html).toContain("Mira Okafor via graph-curator");
  expect(html).toContain(S.review.tokenProposes[p.operation]);
  expect(html).toContain(S.review.tokenActions[`${p.operation}:${p.action}`]);
  expect(html).toContain(S.review.tokenReasons.propose_only);
  // 哪一个 code 都不原样写出来
  expect(html).not.toContain(p.operation);
  expect(html).not.toContain("propose_only");
}

describe("the open proposals of an item, on its card", () => {
  it("renders one chip per proposal, oldest first, each with its operation and action in words", () => {
    const reject = proposed(31, "fact.reject", "reject");
    const close = proposed(32, "fact.close", "close");
    const html = chips([reject, close]);
    ensureReads(html, reject);
    ensureReads(html, close);
    expect(html.split("Mira Okafor via graph-curator").length - 1).toBe(2);
    expect(html.indexOf(S.review.tokenProposes["fact.reject"])).toBeLessThan(
      html.indexOf(S.review.tokenProposes["fact.close"]),
    );
    // 标签，不是控件：答在 Agent 一档
    expect(html).not.toMatch(/<(?:button|input|select|textarea)/);
  });

  it("renders nothing at all without a proposal", () => {
    expect(chips(undefined)).toBe("");
    expect(chips(null)).toBe("");
    expect(chips([])).toBe("");
  });

  it("has words for every operation and action pair, in both languages", () => {
    for (const strings of [en, zh]) {
      for (const pair of Object.keys(en.review.tokenActions)) {
        const operation = pair.split(":")[0];
        expect(strings.review.tokenActions[pair], pair).toBeTruthy();
        expect(strings.review.tokenOperations[operation], operation).toBeTruthy();
      }
    }
    // 只能提议的操作的每一对都渲染得出来，不落回 code
    for (const pair of Object.keys(en.review.tokenActions)) {
      const [operation, action] = pair.split(":");
      if (!S.review.tokenProposes[operation]) continue;
      ensureReads(chips([proposed(33, operation, action)]), proposed(33, operation, action));
    }
  });

  it("keeps a merge or revert proposal as it was: no action, no separator", () => {
    expect(chip(proposal)).toBe(chips([proposal]).replace(/^<div[^>]*>|<\/div>$/g, ""));
    expect(chip(proposal)).toContain(
      `Mira Okafor via graph-curator ${S.review.tokenProposes["review.merge"]}<`,
    );
  });
});

const fact: FactReviewItem = {
  id: `${ID}51`,
  subject_name: "Seed Cy Okoro",
  predicate_label: "seed works at",
  object_name: "Seed Acme",
  valid_from: null,
  valid_to: null,
  confidence: 0.4,
  evidence_count: 1,
  quote: "Seed Cy Okoro works at Seed Acme.",
};

const conflict: ConflictItem = {
  id: `${ID}52`,
  reason: "no_time",
  created_at: "2026-11-18T09:30:00Z",
  predicate_label: "seed works at",
  old_fact_id: `${ID}53`,
  old_subject: "Seed Ada Quill",
  old_object: "Seed Acme",
  old_valid_from: null,
  old_valid_from_precision: null,
  new_fact_id: `${ID}54`,
  new_subject: "Seed Ada Quill",
  new_object: "Seed Globex",
  new_valid_from: null,
  new_valid_from_precision: null,
  new_confidence: 1,
};

const pending: PendingFactItem = {
  id: `${ID}55`,
  subject_id: `${ID}56`,
  subject_name: "Seed Ada Quill",
  predicate_id: null,
  predicate_label: "seed works at",
  proposed_predicate: null,
  phrase: null,
  qualifiers: null,
  time_words: null,
  object_id: null,
  object_name: "Seed Globex",
  object_value: null,
  valid_from: null,
  valid_to: null,
  confidence: 0.6,
  chunk_id: `${ID}57`,
  quote: "Seed Ada Quill moved to Seed Globex.",
  proposed_by: `${ID}58`,
  proposed_by_name: "Tomas Reyes",
  proposed_token_name: "memory-agent",
  created_at: "2026-11-18T09:30:00Z",
};

const violation: AxiomViolation = {
  id: `${ID}59`,
  kind: "asymmetry",
  predicate: "seed manages",
  left_fact: `${ID}60`,
  left_text: "Seed Cy Okoro seed manages Seed Ada Quill",
  right_fact: `${ID}61`,
  right_text: "Seed Ada Quill seed manages Seed Cy Okoro",
  path_len: 0,
  detected_at: "2026-11-18T09:30:00Z",
  detail: {},
  hint: null,
  path: [],
};

const contradiction: AxiomViolation = {
  ...violation,
  id: `${ID}62`,
  kind: "derived_contradiction",
  detail: { subject: "Seed Acme", predicate: "seed part of", object: "Seed Globex" },
};

const defect: OntologyDefect = {
  id: `${ID}63`,
  kind: "symmetric_and_asymmetric",
  subject_label: "seed manages",
  other_label: null,
  path_labels: [],
  detected_at: "2026-11-18T09:30:00Z",
  detail: {},
};

const errata: ErrataItem = {
  id: `${ID}64`,
  document_id: `${ID}65`,
  document: "seed-review-sections.md",
  action: "retract",
  flag: null,
  fact_id: `${ID}66`,
  proposed: { subject: "Seed Cy Okoro", property: "seed works at", object: "Seed Acme" },
  reason: "The passage was replaced.",
  quote: null,
  detail: null,
  created_at: "2026-11-18T09:30:00Z",
};

const phrase: Extract<AlignmentItem, { kind: "phrase" }> = {
  kind: "phrase",
  id: `${ID}67`,
  phrase: "moved to",
  subject_class: "Seed person",
  object_class: "Seed organization",
  object_is_value: false,
  statement_count: 2,
  examples: [],
  votes: null,
  decided_at: "2026-11-18T09:30:00Z",
  bound_to: null,
  direction: null,
};

const rule: Extract<AlignmentItem, { kind: "rule" }> = {
  kind: "rule",
  id: `${ID}68`,
  trigger: "phrase",
  phrase: "moved to",
  subject_class: "Seed person",
  object_class: "Seed organization",
  object_is_value: false,
  property: "seed_works_at",
  property_label: "seed works at",
  reading: null,
  statement_count: 2,
  examples: [],
  votes: null,
  decided_at: "2026-11-18T09:30:00Z",
};

const decision: AgentDecision = {
  id: `${ID}69`,
  run_id: `${ID}70`,
  target_kind: "review",
  target_id: `${ID}71`,
  action: "merge",
  confidence: 0.9,
  reason: null,
  precedents: [],
  status: "proposed",
  merge_id: null,
  question: null,
  trace: [],
  calls: 0,
  created_at: "2026-11-18T09:30:00Z",
  decided_at: null,
  decided_by_name: null,
  left: "Seed Dana Li",
  right: "Seed Dana Li",
};

/** Approach 里列的每一张卡片：怎么渲染它，以及它那一档的提议是哪种操作、哪个动作。
 *  `render` 给了提议就把它挂到条目上再渲染 */
const CARDS: {
  name: string;
  operation: string;
  action: string;
  render: (proposals?: AgentProposal[]) => string;
}[] = [
  {
    name: "FactRow (Low confidence)",
    operation: "fact.confirm",
    action: "confirm",
    render: (p) =>
      renderToStaticMarkup(
        createElement(FactRow, {
          fact: { ...fact, ...(p && { agent_proposals: p }) },
          busy: false,
          onConfirm: noop,
          onReject: noop,
        }),
      ),
  },
  {
    name: "UnconfirmedRow",
    operation: "fact.close",
    action: "close",
    render: (p) =>
      renderToStaticMarkup(
        createElement(UnconfirmedRow, {
          fact: { ...fact, ...(p && { agent_proposals: p }) },
          busy: false,
          onReject: noop,
          onClose: noop,
        }),
      ),
  },
  {
    name: "ConflictRow",
    operation: "conflict.resolve",
    action: "keep",
    render: (p) =>
      renderToStaticMarkup(
        createElement(ConflictRow, {
          conflict: { ...conflict, ...(p && { agent_proposals: p }) },
          busy: false,
          onResolve: noop,
        }),
      ),
  },
  {
    name: "PendingFactRow (Awaiting your nod)",
    operation: "pending.nod",
    action: "confirm",
    render: (p) =>
      renderToStaticMarkup(
        createElement(PendingFactRow, {
          fact: { ...pending, ...(p && { agent_proposals: p }) },
          busy: false,
          canDecide: true,
          onConfirm: noop,
          onReject: noop,
        }),
      ),
  },
  {
    name: "ViolationRow",
    operation: "violation.decide",
    action: "accepted",
    render: (p) =>
      renderToStaticMarkup(
        createElement(ViolationRow, {
          violation: { ...violation, ...(p && { agent_proposals: p }) },
          busy: false,
          onDecide: noop,
          onDuplicates: noop,
          onOntology: noop,
        }),
      ),
  },
  {
    name: "ContradictionRow",
    operation: "violation.decide",
    action: "fact_closed",
    render: (p) =>
      renderToStaticMarkup(
        createElement(ViolationRow, {
          violation: { ...contradiction, ...(p && { agent_proposals: p }) },
          busy: false,
          onDecide: noop,
          onDuplicates: noop,
          onOntology: noop,
        }),
      ),
  },
  {
    name: "DefectRow",
    operation: "defect.decide",
    action: "fixed",
    render: (p) =>
      renderToStaticMarkup(
        createElement(DefectRow, {
          defect: { ...defect, ...(p && { agent_proposals: p }) },
          busy: false,
          onDecide: noop,
        }),
      ),
  },
  {
    name: "ErrataRow",
    operation: "errata.decide",
    action: "approve",
    render: (p) =>
      renderToStaticMarkup(
        createElement(ErrataRow, {
          item: { ...errata, ...(p && { agent_proposals: p }) },
          busy: false,
          onDecide: noop,
        }),
      ),
  },
  {
    name: "AlignmentPhraseRow",
    operation: "alignment.phrase",
    action: "none",
    render: (p) =>
      renderToStaticMarkup(
        createElement(AlignmentPhraseRow, {
          item: { ...phrase, ...(p && { agent_proposals: p }) },
          properties: [],
          busy: false,
          onDecide: noop,
        }),
      ),
  },
  {
    name: "AlignmentRuleRow",
    operation: "alignment.rule",
    action: "approve",
    render: (p) =>
      renderToStaticMarkup(
        createElement(AlignmentRuleRow, {
          item: { ...rule, ...(p && { agent_proposals: p }) },
          busy: false,
          onDecide: noop,
        }),
      ),
  },
  {
    name: "AgentRow (a decision of the governor)",
    operation: "governor.answer",
    action: "keep",
    render: (p) =>
      renderToStaticMarkup(
        createElement(AgentRow, {
          d: { ...decision, ...(p && { agent_proposals: p }) },
          busy: false,
          onAnswer: noop,
        }),
      ),
  },
];

describe("each Review card shows the open proposals of its item", () => {
  for (const card of CARDS) {
    it(`${card.name}: one chip per proposal, and the card is as before without one`, () => {
      const first = proposed(81, card.operation, card.action);
      const plain = card.render();
      // 没有提议的卡片：没有 via，也没有那一行；空的一列与没有这个键渲染得一样
      expect(plain).not.toContain("graph-curator");
      expect(card.render([])).toBe(plain);

      const one = card.render([first]);
      ensureReads(one, first);
      expect(one.split("Mira Okafor via graph-curator").length - 1).toBe(1);
      // 卡片别的地方一个字没动：去掉那一行就是没有提议的卡片
      expect(one.replace(chips([first]), "")).toBe(plain);

      // 同一个目标上开着两条：两个标签
      const second = { ...first, id: `${ID}82`, owner_name: null };
      const two = card.render([first, second]);
      expect(two.split(" via graph-curator").length - 1).toBe(2);
      expect(two).toContain(`${S.kbset.deletedUser} via graph-curator`);
      expect(two.replace(chips([first, second]), "")).toBe(plain);
    });
  }

  it("covers every propose-only operation that has a card in the Review tab", () => {
    const covered = new Set(CARDS.map((card) => card.operation));
    // `fact.reject` 与 `fact.close` 挂在同一张事实卡上；本体提案的卡片在本体页，不在这里
    for (const operation of Object.keys(S.review.tokenProposes)) {
      if (["review.merge", "merge.manual", "merge.revert"].includes(operation)) continue;
      if (["fact.reject", "ontology.proposal"].includes(operation)) continue;
      expect(covered.has(operation), operation).toBe(true);
    }
  });

  it("keeps the line of the nod card that names who remembered the sentence, beside the proposal", () => {
    const nod = CARDS.find((card) => card.operation === "pending.nod")!;
    const said = S.review.pendingSaidVia("Tomas Reyes", "memory-agent");
    expect(nod.render()).toContain(said);
    const html = nod.render([proposed(83, "pending.nod", "confirm")]);
    // 两句都在：谁记下的这句话，和谁（哪枚令牌）提议怎么答它
    expect(html).toContain(said);
    expect(html).toContain("Mira Okafor via graph-curator");
    expect(html).toContain(S.review.tokenProposes["pending.nod"]);
    expect(html).toContain(S.review.tokenActions["pending.nod:confirm"]);
    // 提议不替人点头：点头的两个按钮还在
    expect(html).toContain(`>${S.review.confirm}<`);
    expect(html).toContain(`>${S.review.reject}<`);
  });
});

describe("the Agent rail badge", () => {
  it("adds the open token proposals to the open proposals of the governor", () => {
    expect(agentRailCount({ agent: 2, agent_token_proposals: 3 })).toBe(5);
    expect(agentRailCount({ agent: 0, agent_token_proposals: 4 })).toBe(4);
  });

  it("is the count of the governor when no token proposal is open, and zero before the counts load", () => {
    // 没有开着的令牌提议时回答里没有这个键
    expect(agentRailCount({ agent: 2 })).toBe(2);
    expect(agentRailCount({ agent: 0 })).toBe(0);
    expect(agentRailCount(undefined)).toBe(0);
  });
});
