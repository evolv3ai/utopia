import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AgentProposal, MergeLog } from "../api";
import { S } from "../i18n";
import { AgentProposalChip, MergeActor } from "./AgentProposal";

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
