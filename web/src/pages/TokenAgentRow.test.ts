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
];
const PROPOSABLE = ["review.merge", "merge.manual", "merge.revert"];
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
  });

  it("marks the client as declared", () => {
    expect(declaredClientLabel("curl/8.5.0")).toBe("client (declared): curl/8.5.0");
  });
});
