import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TokenAgentAction } from "../api";
import { S } from "../i18n";
import { en } from "../i18n/en";
import { zh } from "../i18n/zh";
import {
  TokenAgentRow,
  activityTokenLabel,
  batchLabel,
  declaredClientLabel,
  isOpenProposal,
  tokenActionLabel,
  tokenOperationLabel,
  tokenProposalLabel,
  tokenReasonLabel,
  tokenStateLabel,
  viaToken,
} from "./TokenAgentRow";

const keep: TokenAgentAction = {
  source: "token",
  id: "019a0000-0000-7000-8000-000000000001",
  operation: "review.keep",
  target_kind: "review",
  target_id: "019a0000-0000-7000-8000-000000000002",
  owner_id: "019a0000-0000-7000-8000-000000000003",
  owner_name: "Mira Okafor",
  token_id: "019a0000-0000-7000-8000-000000000004",
  token_name: "graph-curator",
  outcome: "applied",
  left: "Ada Lovelace",
  right: "Ada King",
  rationale: "different people",
  created_at: "2026-11-18T09:30:00Z",
};

/** 闸门留下的一次队列合并：一条开着的提议 */
const heldMerge: TokenAgentAction = {
  ...keep,
  id: "019a0000-0000-7000-8000-000000000011",
  operation: "review.merge",
  outcome: "held",
  reason: "kept_by_person",
  rationale: "same person",
};

/** 令牌想撤一次人做的合并：一条开着的提议，有方向（源并进目标） */
const proposedRevert: TokenAgentAction = {
  ...keep,
  id: "019a0000-0000-7000-8000-000000000012",
  operation: "merge.revert",
  target_kind: "merge",
  outcome: "proposed",
  reason: "merge_by_person",
  merge_id: "019a0000-0000-7000-8000-000000000013",
  rationale: null,
};

const render = (a: TokenAgentAction) => renderToStaticMarkup(createElement(TokenAgentRow, { a }));
const noop = () => {};
const renderAnswerable = (a: TokenAgentAction) =>
  renderToStaticMarkup(
    createElement(TokenAgentRow, { a, onApprove: noop, onDecline: noop, onPick: noop }),
  );

/** Review 其余各档里只能提议的十二种操作（0076）：令牌的调用什么都不改，只留一条提议 */
const PROPOSE_ONLY = [
  "fact.confirm",
  "fact.reject",
  "fact.close",
  "conflict.resolve",
  "violation.decide",
  "defect.decide",
  "errata.decide",
  "alignment.phrase",
  "alignment.rule",
  "ontology.proposal",
  "pending.nod",
  "governor.answer",
];
/** 直接落地的四种 */
const FINALIZED = ["mapping.decide", "alignment.kind_word", "consistency.check", "inference.run"];
/** 这些操作在 Activity 里留下的审计动作 */
const TOKEN_AUDIT_ACTIONS = [
  "mapping.decided",
  "consistency.checked",
  "inference.materialized",
  "alignment.kind_word_decided",
  "ontology_proposal.rejected",
];
/** 这一刀服务端会写出来的每一个 code：动作、理由、后来的状态、结果 */
const OPERATIONS = [
  "review.keep",
  "review.merge",
  "merge.manual",
  "merge.revert",
  // Activity 里与提议有关的三种事件（审计的 action）
  "agent.proposal",
  "agent.proposal.declined",
  "agent.proposal.superseded",
  ...PROPOSE_ONLY,
  ...FINALIZED,
  ...TOKEN_AUDIT_ACTIONS,
];
const PROPOSABLE = ["review.merge", "merge.manual", "merge.revert", ...PROPOSE_ONLY];
/** 每一种操作能带的动作（台账 payload 里的 `action`）。两种运行没有动作 */
const ACTIONS: Record<string, string[]> = {
  "fact.confirm": ["confirm"],
  "fact.reject": ["reject"],
  "fact.close": ["close"],
  "conflict.resolve": ["close", "retime_new", "keep", "reject_new"],
  "violation.decide": ["fact_retracted", "fact_closed", "axiom_relaxed", "accepted"],
  "defect.decide": ["fixed", "accepted"],
  "errata.decide": ["approve", "reject"],
  "alignment.rule": ["approve", "reject"],
  "alignment.phrase": ["bind", "none"],
  "alignment.kind_word": ["bind", "none"],
  "ontology.proposal": ["adopt", "reject"],
  "pending.nod": ["confirm", "reject"],
  "governor.answer": ["merge", "keep", "revert"],
  "mapping.decide": ["confirmed", "rejected"],
};
const ACTION_PAIRS = Object.entries(ACTIONS).flatMap(([operation, actions]) =>
  actions.map((action) => `${operation}:${action}`),
);
const REASONS = [
  "identity",
  "contradiction",
  "derived",
  "answered",
  "kept_by_person",
  "reverted_merge",
  "merge_by_person",
  "merge_by_governor",
  "breaker",
  "propose_only",
];
const STATES = ["approved", "declined", "superseded", "undone"];
const OUTCOMES = ["applied", "held", "proposed"];

describe("a token agent's row in Agent History", () => {
  it("reads owner via token name, the action in words, and both names", () => {
    const html = render(keep);
    expect(html).toContain("Mira Okafor via graph-curator");
    expect(html).toContain(S.review.tokenOperations["review.keep"]);
    expect(html).not.toContain("review.keep");
    expect(html).toContain("Ada Lovelace");
    expect(html).toContain("Ada King");
    expect(html).toContain(S.review.tokenOutcome.applied);
    expect(html).toContain("different people");
    expect(html).toContain("2026");
  });

  it("has no Merge, Keep or Revert button, and no button at all", () => {
    const html = render(keep);
    // 写成正则：风格守卫不许源码里出现原生控件的字面量
    expect(html).not.toMatch(/<(?:button|input|select|textarea)/);
    for (const label of [S.review.merge, S.review.keep, S.review.revert]) {
      expect(html).not.toContain(`>${label}<`);
    }
    // 开着的提议不给处理函数（Viewer 看到的）：同样一个控件都没有
    const open = render(heldMerge);
    expect(open).not.toMatch(/<(?:button|input|select|textarea)/);
    for (const label of [
      S.review.merge,
      S.review.keep,
      S.review.revert,
      S.review.tokenApprove,
      S.review.tokenDecline,
    ]) {
      expect(open).not.toContain(">" + label + "<");
    }
  });

  it("leaves the rationale out when there is none", () => {
    const html = render({ ...keep, rationale: null });
    expect(html).toContain("Mira Okafor via graph-curator");
    expect(html).not.toContain("“");
  });

  it("uses the removed-user wording when the owner's name is gone", () => {
    const html = render({ ...keep, owner_name: null });
    expect(html).toContain(`${S.kbset.deletedUser} via graph-curator`);
  });
});

describe("a batch member in Agent History", () => {
  const batchId = "019a0000-0000-7000-8000-0000a1b2c3d4";
  const other = "019a0000-0000-7000-8000-00005e6f7a8b";

  it("reads owner via token name and carries a short batch label", () => {
    for (const a of [
      { ...keep, batch_id: batchId },
      { ...keep, operation: "review.merge", merge_id: proposedRevert.merge_id, batch_id: batchId },
      { ...heldMerge, batch_id: batchId },
      { ...keep, operation: "merge.revert", target_kind: "merge", batch_id: batchId },
    ]) {
      const html = render(a);
      expect(html).toContain("Mira Okafor via graph-curator");
      expect(html).toContain(batchLabel(batchId));
      expect(html).toContain("batch a1b2c3d4");
      // 整个号只在 title 里，行上写的是短标签
      expect(html).not.toContain(`>${batchId}<`);
    }
  });

  it("tells two batches apart and writes no label on a single call", () => {
    expect(batchLabel(batchId)).not.toBe(batchLabel(other));
    expect(render({ ...keep, batch_id: other })).toContain("batch 5e6f7a8b");
    expect(render(keep)).not.toContain("batch");
    expect(render(heldMerge)).not.toContain("batch");
  });

  it("adds no control: a batch is not reverted from the Review tab", () => {
    const html = renderAnswerable({ ...keep, operation: "review.merge", batch_id: batchId });
    expect(html).not.toMatch(/<(?:button|input|select|textarea)/);
    expect(html).not.toContain(`>${S.review.revert}<`);
  });
});

describe("an open proposal in Agent History", () => {
  it("renders Approve and Decline when the handlers are given", () => {
    for (const a of [heldMerge, proposedRevert]) {
      const html = renderAnswerable(a);
      expect(html).toContain(`>${S.review.tokenApprove}<`);
      expect(html).toContain(`>${S.review.tokenDecline}<`);
      // 写成正则的分组：风格守卫不许源码里出现原生控件的字面量
      expect(html).toMatch(/<(?:button)/);
      // 可以勾进批量回答
      expect(html).toContain(S.review.tokenPickProposal);
    }
  });

  it("says what the agent wants, why it was held, and shows no code raw", () => {
    const html = renderAnswerable(heldMerge);
    expect(html).toContain("Mira Okafor via graph-curator");
    expect(html).toContain(S.review.tokenProposes["review.merge"]);
    expect(html).toContain(S.review.tokenReasons.kept_by_person);
    expect(html).toContain(S.review.tokenOutcome.held);
    expect(html).not.toContain("review.merge");
    expect(html).not.toContain("kept_by_person");
    expect(html).not.toContain(">held<");

    const revert = renderAnswerable(proposedRevert);
    expect(revert).toContain(S.review.tokenProposes["merge.revert"]);
    expect(revert).toContain(S.review.tokenReasons.merge_by_person);
    expect(revert).not.toContain("merge.revert");
    expect(revert).not.toContain("merge_by_person");
  });

  it("writes the direction of a manual merge and of a revert, source to target", () => {
    expect(renderAnswerable(proposedRevert)).toContain("Ada Lovelace → Ada King");
    expect(
      renderAnswerable({ ...heldMerge, operation: "merge.manual", target_kind: "entity" }),
    ).toContain("Ada Lovelace → Ada King");
    // 队列里的一对没有方向
    expect(renderAnswerable(heldMerge)).toContain("Ada Lovelace ≟ Ada King");
  });

  it("loses its controls once it is answered and shows the state in words", () => {
    for (const state of ["approved", "declined", "superseded"]) {
      const html = renderAnswerable({ ...heldMerge, state, state_at: "2026-11-19T10:00:00Z" });
      expect(html).not.toMatch(/<(?:button|input|select|textarea)/);
      expect(html).not.toContain(`>${S.review.tokenApprove}<`);
      expect(html).toContain(S.review.tokenStates[state]);
      expect(html).not.toContain(`>${state}<`);
    }
  });

  it("shows an applied merge that a person reverted as undone, with no control", () => {
    const html = renderAnswerable({
      ...keep,
      operation: "merge.manual",
      merge_id: "019a0000-0000-7000-8000-000000000013",
      state: "undone",
      state_at: "2026-11-19T10:00:00Z",
    });
    expect(html).toContain(S.review.tokenOperations["merge.manual"]);
    expect(html).toContain(S.review.tokenStates.undone);
    expect(html).not.toMatch(/<(?:button|input|select|textarea)/);
  });

  it("is open only while it is held or proposed and has no state", () => {
    expect(isOpenProposal(heldMerge)).toBe(true);
    expect(isOpenProposal(proposedRevert)).toBe(true);
    expect(isOpenProposal(keep)).toBe(false);
    expect(isOpenProposal({ ...heldMerge, state: "declined" })).toBe(false);
  });
});

/** 令牌想裁一处本体缺陷：目标不是一对，服务端给的是一句话和一个动作 */
const proposedDefect: TokenAgentAction = {
  ...keep,
  id: "019a0000-0000-7000-8000-000000000031",
  operation: "defect.decide",
  target_kind: "ontology_defect",
  outcome: "proposed",
  reason: "propose_only",
  left: null,
  right: null,
  rationale: null,
  target_label: "symmetric_and_asymmetric: seed manages",
  action: "fixed",
};

describe("a row whose target is not a pair", () => {
  it("names its target in words and prints no placeholder for a missing side", () => {
    const html = renderAnswerable(proposedDefect);
    expect(html).toContain("Mira Okafor via graph-curator");
    expect(html).toContain("symmetric_and_asymmetric: seed manages");
    expect(html).toContain(S.review.tokenProposes["defect.decide"]);
    expect(html).toContain(S.review.tokenActions["defect.decide:fixed"]);
    expect(html).toContain(S.review.tokenReasons.propose_only);
    expect(html).not.toContain("?");
    expect(html).not.toContain("≟");
    expect(html).not.toContain("→");
    expect(html).not.toContain("defect.decide");
    expect(html).not.toContain("propose_only");
    expect(html).not.toContain(">fixed<");
    // 开着的提议：答得了
    expect(html).toContain(`>${S.review.tokenApprove}<`);
    expect(html).toContain(`>${S.review.tokenDecline}<`);
  });

  it("does so for every operation of the ledger and every action it can carry", () => {
    for (const operation of [...PROPOSE_ONLY, ...FINALIZED]) {
      const proposed = PROPOSE_ONLY.includes(operation);
      for (const action of ACTIONS[operation] ?? [undefined]) {
        const a: TokenAgentAction = {
          ...proposedDefect,
          operation,
          outcome: proposed ? "proposed" : "applied",
          reason: proposed ? "propose_only" : undefined,
          target_label: "Seed Ada Quill works at Seed Acme",
          action,
        };
        const html = render(a);
        expect(html, operation).toContain("Mira Okafor via graph-curator");
        expect(html, operation).toContain("Seed Ada Quill works at Seed Acme");
        expect(html, operation).toContain(
          proposed ? S.review.tokenProposes[operation] : S.review.tokenOperations[operation],
        );
        if (action) {
          expect(html, `${operation}:${action}`).toContain(
            S.review.tokenActions[`${operation}:${action}`],
          );
          expect(html).not.toContain(`: ${action}<`);
        }
        expect(html, operation).not.toContain("?");
        expect(html, operation).not.toContain(operation);
        expect(html, operation).not.toContain("propose_only");
      }
    }
  });

  it("prints nothing where the target would be when the row has no label and no sides", () => {
    const bare: TokenAgentAction = { ...proposedDefect };
    delete bare.target_label;
    const html = render(bare);
    expect(html).not.toContain("?");
    expect(html).not.toContain("≟");
    // 一对的行少了一侧，照旧写问号：那是「这一侧的名字读不到了」
    expect(render({ ...keep, right: null })).toContain("Ada Lovelace ≟ ?");
  });

  it("keeps a pair row as it was: both names, no target label", () => {
    expect(render(keep)).toContain("Ada Lovelace ≟ Ada King");
    expect(renderAnswerable(heldMerge)).toContain("Ada Lovelace ≟ Ada King");
  });
});

describe("an approved alignment change that is only queued", () => {
  for (const [lang, strings] of [
    ["en", en],
    ["zh", zh],
  ] as const) {
    it(`${lang}: has a queued wording that is not the approved or the applied wording`, () => {
      const r = strings.review;
      expect(r.tokenQueued).toBeTruthy();
      expect(r.tokenQueued).not.toBe(r.tokenStates.approved);
      expect(r.tokenQueued).not.toBe(r.tokenOutcome.applied);
      expect(r.tokenQueued).not.toContain(r.tokenOutcome.applied);
    });
  }

  it("reads the queued wording on an approved phrase or rule row, not the approved one", () => {
    for (const [operation, action] of [
      ["alignment.phrase", "bind"],
      ["alignment.rule", "approve"],
    ]) {
      const approved: TokenAgentAction = {
        ...proposedDefect,
        operation,
        action,
        target_label: "moved to (Seed person → Seed organization)",
        state: "approved",
        state_at: "2026-11-19T10:00:00Z",
        queued: true,
      };
      const html = renderAnswerable(approved);
      expect(html).toContain(`>${S.review.tokenQueued}<`);
      expect(html).not.toContain(`>${S.review.tokenStates.approved}<`);
      expect(html).not.toContain(`>${S.review.tokenOutcome.applied}<`);
      expect(html).not.toMatch(/<(?:button|input|select|textarea)/);
      // 没有 `queued` 的批准（别的操作）照旧写「已批准」
      const plain = renderAnswerable({ ...approved, queued: undefined });
      expect(plain).toContain(`>${S.review.tokenStates.approved}<`);
      expect(plain).not.toContain(S.review.tokenQueued);
      // 还开着、或者被拒绝的行不会写成排着
      expect(renderAnswerable({ ...approved, state: "declined" })).not.toContain(
        S.review.tokenQueued,
      );
    }
  });
});

describe("every code the token ledger writes has a label in both languages", () => {
  for (const [lang, strings] of [
    ["en", en],
    ["zh", zh],
  ] as const) {
    it(`${lang}: operations, proposals, reasons, states and outcomes`, () => {
      const r = strings.review;
      for (const [table, codes] of [
        [r.tokenOperations, OPERATIONS],
        [r.tokenProposes, PROPOSABLE],
        [r.tokenReasons, REASONS],
        [r.tokenStates, STATES],
        [r.tokenOutcome, OUTCOMES],
      ] as const) {
        for (const code of codes) {
          expect(table[code], `${lang}: ${code}`).toBeTruthy();
          expect(table[code]).not.toBe(code);
        }
      }
      // 操作与动作的每一对：有说法，说法不是那个 code 本身
      expect(Object.keys(r.tokenActions).sort()).toEqual([...ACTION_PAIRS].sort());
      for (const pair of ACTION_PAIRS) {
        const action = pair.split(":")[1];
        expect(r.tokenActions[pair], `${lang}: ${pair}`).toBeTruthy();
        expect(r.tokenActions[pair]).not.toBe(pair);
        expect(r.tokenActions[pair]).not.toBe(action);
      }
      // 只能提议的操作：哪一张表里的说法都不把它写成令牌自己做了的事
      for (const code of PROPOSE_ONLY) {
        expect(r.tokenProposes[code], `${lang}: ${code}`).toBeTruthy();
      }
      expect(r.tokenWithAction("what", "action")).toContain("what");
      expect(r.tokenWithAction("what", "action")).toContain("action");
      expect(r.tokenBatch("a1b2c3d4")).toContain("a1b2c3d4");
      expect(r.tokenBatch("a1b2c3d4")).not.toBe("a1b2c3d4");
      for (const text of [
        r.tokenApprove,
        r.tokenDecline,
        r.tokenApproveSelected,
        r.tokenDeclineSelected,
        r.tokenSelectOpen,
        r.tokenPickProposal,
      ]) {
        expect(text).toBeTruthy();
      }
    });
  }

  it("falls back to the code itself only for one it does not know", () => {
    for (const code of REASONS) expect(tokenReasonLabel(code)).not.toBe(code);
    for (const code of STATES) expect(tokenStateLabel(code)).not.toBe(code);
    for (const code of PROPOSABLE) expect(tokenProposalLabel(code)).not.toBe(code);
    // 少一个标签，下面这一行就失败：标签函数对不认得的 code 原样写出来
    for (const code of OPERATIONS) expect(tokenOperationLabel(code)).not.toBe(code);
    for (const code of OUTCOMES) expect(S.review.tokenOutcome[code]).toBeTruthy();
    expect(tokenReasonLabel("something_new")).toBe("something_new");
    expect(tokenStateLabel("something_new")).toBe("something_new");
    for (const pair of ACTION_PAIRS) {
      const [operation, action] = pair.split(":");
      expect(tokenActionLabel(operation, action)).toBe(S.review.tokenActions[pair]);
      expect(tokenActionLabel(operation, action)).not.toBe(action);
    }
    expect(tokenActionLabel("fact.confirm", "something_new")).toBe("something_new");
  });

  it("covers the sixteen operations this milestone writes and their five audit actions", () => {
    expect(new Set([...PROPOSE_ONLY, ...FINALIZED]).size).toBe(16);
    expect(TOKEN_AUDIT_ACTIONS.length).toBe(5);
    expect(REASONS).toContain("propose_only");
  });
});

describe("the labels token rows share", () => {
  it("writes owner via token name", () => {
    expect(viaToken("Mira Okafor", "graph-curator")).toBe("Mira Okafor via graph-curator");
    expect(viaToken(null, "graph-curator")).toBe(`${S.kbset.deletedUser} via graph-curator`);
  });

  it("names a known operation in words and leaves an unknown one as it is", () => {
    expect(tokenOperationLabel("review.keep")).toBe(S.review.tokenOperations["review.keep"]);
    expect(tokenOperationLabel("review.keep")).not.toBe("review.keep");
    expect(tokenOperationLabel("review.something_new")).toBe("review.something_new");
  });

  it("labels every token event in Activity, and a person's answer to a proposal", () => {
    for (const code of OPERATIONS) {
      expect(activityTokenLabel(code, true)).toBe(tokenOperationLabel(code));
      expect(activityTokenLabel(code, true)).not.toBe(code);
    }
    // 人的行：只有答提议的那两种动作在这张表里，别的照旧走 Activity 自己的表
    for (const code of ["agent.proposal.declined", "agent.proposal.superseded"]) {
      expect(activityTokenLabel(code, false)).toBe(S.review.tokenOperations[code]);
    }
    for (const code of ["review.keep", "review.merge", "merge.revert", "kb.updated"]) {
      expect(activityTokenLabel(code, false)).toBeNull();
    }
    // 人批准令牌的提议、拒绝一条本体提案：这个动作只有那时才写，也在这张表里
    expect(activityTokenLabel("ontology_proposal.rejected", false)).toBe(
      S.review.tokenOperations["ontology_proposal.rejected"],
    );
    // 人自己做的那几种（映射、检查、推理、类别词）没有新标签：照旧走 Activity 自己的表
    for (const code of [
      "mapping.decided",
      "consistency.checked",
      "inference.materialized",
      "alignment.kind_word_decided",
    ]) {
      expect(activityTokenLabel(code, false)).toBeNull();
      expect(activityTokenLabel(code, true)).toBe(S.review.tokenOperations[code]);
    }
  });

  it("marks the client as declared", () => {
    expect(declaredClientLabel("curl/8.5.0")).toBe("client (declared): curl/8.5.0");
  });
});
