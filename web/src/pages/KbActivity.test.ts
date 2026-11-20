import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { ApiError, type AuditEvent } from "../api";
import { S } from "../i18n";
import { KbActivity } from "./KbSettings";
import { batchLabel } from "./TokenAgentRow";

const kbId = "research";
const queryKey = ["kbAudit", kbId, "", "", "", 0];
const empty = { events: [], total: 0, actions: [] };

function createClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false, retryOnMount: false, refetchOnMount: false } },
  });
}

function render(client: QueryClient) {
  return renderToStaticMarkup(createElement(QueryClientProvider, {
    client,
    children: createElement(KbActivity, { kbId }),
  }));
}

it("shows loading without a zero count before the first response", () => {
  const html = render(createClient());
  expect(html).toContain(S.nav.loading);
  expect(html).not.toContain(S.kbset.auditTotal(0));
  expect(html).not.toContain(S.kbset.activityEmpty);
});

it.each([false, true])("offers retry after a 503 (previous empty response: %s)", async (cached) => {
  const client = createClient();
  if (cached) client.setQueryData(queryKey, empty);

  const error = new ApiError(503, "Service Unavailable");
  await expect(client.fetchQuery({
    queryKey,
    queryFn: async () => { throw error; },
  })).rejects.toBe(error);

  const html = render(client);
  expect(html).toContain('role="alert"');
  expect(html).toContain(S.kbset.activityLoadError);
  expect(html).toContain(S.kbset.activityRetry);
  expect(html).not.toContain(S.kbset.auditTotal(0));
  expect(html).not.toContain(S.kbset.activityEmpty);
});

it("shows zero records after a successful empty response", () => {
  const client = createClient();
  client.setQueryData(queryKey, empty);
  const html = render(client);
  expect(html).toContain(S.kbset.auditTotal(0));
  expect(html).toContain(S.kbset.activityEmpty);
  expect(html).not.toContain('role="alert"');
});

// 令牌分开的一对（0076）：actor 是主人，另带令牌的 id、名字快照，和调用方自己报的客户端
const tokenKeep: AuditEvent = {
  id: "019a0000-0000-7000-8000-000000000001",
  action: "review.keep",
  target_kind: "review",
  target_id: "019a0000-0000-7000-8000-000000000002",
  detail: { left: "Ada Lovelace", right: "Ada King", score: 0.5, why: null },
  actor_id: "019a0000-0000-7000-8000-000000000003",
  actor_name: "Mira Okafor",
  created_at: "2026-11-18T09:30:00Z",
  token_id: "019a0000-0000-7000-8000-000000000004",
  token_name: "graph-curator",
  declared_client: "curl/8.5.0",
};

function renderEvents(events: AuditEvent[]) {
  const client = createClient();
  client.setQueryData(queryKey, { events, total: events.length, actions: ["review.keep"] });
  return render(client);
}

/** 只看台账那几行：筛选下拉也会写出动作名，那不是行上的标签 */
function rowsOf(html: string) {
  const start = html.indexOf("divide-y");
  expect(start).toBeGreaterThan(-1);
  return html.slice(start);
}

it("shows a token event as owner via token name with a readable action and a declared client", () => {
  const rows = rowsOf(renderEvents([tokenKeep]));
  expect(rows).toContain("Mira Okafor via graph-curator");
  expect(rows).toContain(S.review.tokenOperations["review.keep"]);
  expect(rows).not.toContain("review.keep");
  // 客户端是调用方自己报的：只以「declared」的写法出现，不单独当身份写出来
  expect(rows).toContain("client (declared): curl/8.5.0");
  expect(rows.split("curl/8.5.0").length - 1).toBe(1);
});

it("shows a token event that sent no User-Agent without a client label", () => {
  const withoutClient: AuditEvent = { ...tokenKeep };
  delete withoutClient.declared_client;
  const rows = rowsOf(renderEvents([withoutClient]));
  expect(rows).toContain("Mira Okafor via graph-curator");
  expect(rows).toContain(S.review.tokenOperations["review.keep"]);
  expect(rows).not.toContain("review.keep");
  expect(rows).not.toContain("declared");
  expect(rows).not.toContain("client");
});

it("uses the removed-user wording before via when the owner's name is gone", () => {
  const rows = rowsOf(renderEvents([{ ...tokenKeep, actor_name: null }]));
  expect(rows).toContain(`${S.kbset.deletedUser} via graph-curator`);
});

// 令牌成批调用的一笔（0076）：另带服务端发的那一批的号
const batchId = "019a0000-0000-7000-8000-0000a1b2c3d4";

/** 令牌写进 Activity 的每一种事件：落地的四种动作、留成提议的那一种，以及 Review 其余
 *  各档直接落地的操作留下的四种审计动作（0076） */
const TOKEN_ACTIONS = [
  "review.keep",
  "review.merge",
  "merge.manual",
  "merge.revert",
  "agent.proposal",
  "mapping.decided",
  "consistency.checked",
  "inference.materialized",
  "alignment.kind_word_decided",
];

/** 只能提议的十二种操作：每一种的提议在 Activity 里都是一行 `agent.proposal` */
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

it("shows every token action as owner via token name in words, never as a raw code", () => {
  for (const action of TOKEN_ACTIONS) {
    const rows = rowsOf(renderEvents([{ ...tokenKeep, action }]));
    expect(rows).toContain("Mira Okafor via graph-curator");
    expect(S.review.tokenOperations[action], action).toBeTruthy();
    expect(rows).toContain(S.review.tokenOperations[action]);
    expect(rows).not.toContain(action);
    // 不是治理 agent 或引擎的行
    expect(rows).not.toContain(S.kbset.adjudicator);
    expect(rows).not.toContain(S.kbset.engine);
  }
});

it("shows a proposal of every operation as owner via the token name stored with the action", () => {
  for (const operation of PROPOSE_ONLY) {
    const proposal: AuditEvent = {
      ...tokenKeep,
      action: "agent.proposal",
      target_kind: "agent_action",
      // 审计行里存的是动作那一刻的令牌名：令牌后来改了名字，这一行不变
      detail: {
        operation,
        outcome: "proposed",
        reason: "propose_only",
        token_id: tokenKeep.token_id,
        token_name: "graph-curator",
        action_id: "019a0000-0000-7000-8000-000000000041",
      },
    };
    const rows = rowsOf(renderEvents([proposal]));
    expect(rows, operation).toContain("Mira Okafor via graph-curator");
    expect(rows, operation).toContain(S.review.tokenOperations["agent.proposal"]);
    // 哪一个 code 都不原样写出来
    expect(rows, operation).not.toContain("agent.proposal");
    expect(rows, operation).not.toContain(operation);
    expect(rows, operation).not.toContain("propose_only");
  }
});

it("shows a person's rejection of an ontology proposal, made by approving an agent's proposal, in words", () => {
  const rejected: AuditEvent = {
    ...tokenKeep,
    action: "ontology_proposal.rejected",
    target_kind: "ontology_proposal",
    detail: { section: "entity_types", agent_proposal: "019a0000-0000-7000-8000-000000000041" },
  };
  delete rejected.token_id;
  delete rejected.token_name;
  delete rejected.declared_client;
  const rows = rowsOf(renderEvents([rejected]));
  expect(rows).toContain("Mira Okafor");
  // 批准之后做决定的是这个人：不写成令牌做的
  expect(rows).not.toContain(" via ");
  expect(rows).toContain(S.review.tokenOperations["ontology_proposal.rejected"]);
  expect(rows).not.toContain("ontology_proposal.rejected");
});

it("shows the batch label on a batch member and none on a single call", () => {
  for (const action of TOKEN_ACTIONS) {
    const rows = rowsOf(renderEvents([{ ...tokenKeep, action, batch_id: batchId }]));
    expect(rows).toContain("Mira Okafor via graph-curator");
    expect(rows).toContain(batchLabel(batchId));
    expect(rows).toContain("batch a1b2c3d4");
  }
  expect(rowsOf(renderEvents([tokenKeep]))).not.toContain("batch");
  // 人的事件哪怕带着这个键也不写：批是令牌的
  const person: AuditEvent = { ...tokenKeep, batch_id: batchId };
  delete person.token_id;
  delete person.token_name;
  expect(rowsOf(renderEvents([person]))).not.toContain("batch");
});

it("shows a person's answer to an agent proposal in words, as that person's", () => {
  for (const action of ["agent.proposal.declined", "agent.proposal.superseded"]) {
    const answered: AuditEvent = {
      ...tokenKeep,
      action,
      target_kind: "agent_action",
      detail: { operation: "review.merge", proposed_by_token: "graph-curator" },
    };
    delete answered.token_id;
    delete answered.token_name;
    delete answered.declared_client;
    const rows = rowsOf(renderEvents([answered]));
    expect(rows).toContain("Mira Okafor");
    expect(rows).not.toContain(" via ");
    expect(rows).toContain(S.review.tokenOperations[action]);
    expect(rows).not.toContain(action);
  }
});

it("leaves events without a token name exactly as they were", () => {
  const base = {
    target_kind: "review",
    target_id: null,
    detail: {},
    created_at: "2026-11-18T09:30:00Z",
  };
  const rows = rowsOf(renderEvents([
    // 人自己分开的一对：没有令牌，哪怕带着别的字段也不写 via、不写客户端
    { ...base, id: "e1", action: "review.keep", actor_id: "u1", actor_name: "Mira Okafor" },
    // 治理 agent / 裁决器、引擎、已移除的用户：标签都不变
    { ...base, id: "e2", action: "review.merge", actor_id: null, actor_name: null },
    { ...base, id: "e3", action: "kb.updated", actor_id: null, actor_name: null },
    { ...base, id: "e4", action: "kb.updated", actor_id: "u2", actor_name: null },
    // 只有令牌 id、没有名字快照的老行（0014 的令牌写入）：不是这一刀的令牌行
    {
      ...base,
      id: "e5",
      action: "document.deleted",
      actor_id: "u1",
      actor_name: "Mira Okafor",
      token_id: "t1",
      declared_client: "curl/8.5.0",
    },
  ]));
  expect(rows).not.toContain(" via ");
  expect(rows).not.toContain("declared");
  expect(rows).not.toContain("curl/8.5.0");
  expect(rows).toContain("review.keep");
  expect(rows).not.toContain(S.review.tokenOperations["review.keep"]);
  expect(rows).toContain(S.kbset.adjudicator);
  expect(rows).toContain(S.kbset.engine);
  expect(rows).toContain(S.kbset.deletedUser);
  expect(rows).toContain(S.kbset.auditActions["kb.updated"]);
  expect(rows).toContain(S.kbset.auditActions["document.deleted"]);
  expect(S.kbset.auditActions["review.keep"]).toBeUndefined();
});
