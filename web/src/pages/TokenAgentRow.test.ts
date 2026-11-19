import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TokenAgentAction } from "../api";
import { S } from "../i18n";
import {
  TokenAgentRow,
  declaredClientLabel,
  tokenOperationLabel,
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

const render = (a: TokenAgentAction) => renderToStaticMarkup(createElement(TokenAgentRow, { a }));

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

  it("marks the client as declared", () => {
    expect(declaredClientLabel("curl/8.5.0")).toBe("client (declared): curl/8.5.0");
  });
});
