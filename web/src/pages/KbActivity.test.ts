import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { ApiError, type AuditEvent } from "../api";
import { S } from "../i18n";
import { zh } from "../i18n/zh";
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

/** 人答令牌提议的一行（0076 决定 8）：actor 是这个人，服务端从台账读出提议它的令牌与主人 */
function answeredBy(
  action: string,
  detail: Record<string, unknown>,
  target_kind = "agent_action",
): AuditEvent {
  const answered: AuditEvent = {
    ...tokenKeep,
    action,
    target_kind,
    detail,
    proposed_by_token: "graph-curator",
    proposed_by_owner: "Mira Okafor",
  };
  delete answered.token_id;
  delete answered.token_name;
  delete answered.declared_client;
  return answered;
}

/** actor 那一格只有这个人：「via」不在动作之前 */
const PERSON_ALONE = '<span class="text-ink">Mira Okafor</span>';

it("shows a person's rejection of an ontology proposal, made by approving an agent's proposal, in words", () => {
  const rejected = answeredBy(
    "ontology_proposal.rejected",
    { section: "entity_types", agent_proposal: "019a0000-0000-7000-8000-000000000041" },
    "ontology_proposal",
  );
  const rows = rowsOf(renderEvents([rejected]));
  // 批准之后做决定的是这个人：actor 不写成令牌做的，提议它的令牌写在动作后面
  expect(rows).toContain(PERSON_ALONE);
  const label = S.review.tokenOperations["ontology_proposal.rejected"];
  expect(rows).toContain(label);
  expect(rows).not.toContain("ontology_proposal.rejected");
  const proposer = S.kbset.auditProposedBy("Mira Okafor via graph-curator");
  expect(rows.indexOf(proposer)).toBeGreaterThan(rows.indexOf(label));
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
    const answered = answeredBy(action, {
      operation: "review.merge",
      proposed_by_token: "graph-curator",
    });
    const rows = rowsOf(renderEvents([answered]));
    expect(rows).toContain(PERSON_ALONE);
    expect(rows).toContain(S.kbset.auditProposedBy("Mira Okafor via graph-curator"));
    expect(rows).toContain(S.review.tokenOperations[action]);
    expect(rows).not.toContain(action);
  }
});

it("names the token whose proposal a person declined, closed or approved", () => {
  const proposal = "019a0000-0000-7000-8000-000000000041";
  const answers: [AuditEvent, string][] = [
    [
      answeredBy("agent.proposal.declined", {
        agent_proposal: proposal,
        operation: "violation.decide",
        proposed_by_token: "graph-curator",
      }),
      S.review.tokenOperations["agent.proposal.declined"],
    ],
    [
      answeredBy("agent.proposal.superseded", {
        agent_proposal: proposal,
        operation: "violation.decide",
        proposed_by_token: "graph-curator",
      }),
      S.review.tokenOperations["agent.proposal.superseded"],
    ],
    // 批准：这个人自己的决定行，动作照旧（不给人的 Review 动作加标签，D128）
    [
      answeredBy(
        "violation.decided",
        { resolution: "accepted", agent_proposal: proposal },
        "axiom_violation",
      ),
      "violation.decided",
    ],
  ];
  for (const [event, label] of answers) {
    const rows = rowsOf(renderEvents([event]));
    // actor 那一格只有这个人
    expect(rows, event.action).toContain(PERSON_ALONE);
    expect(rows, event.action).toContain(label);
    // 动作之后写出提议它的是谁：主人 via 存下的令牌名
    const proposer = S.kbset.auditProposedBy("Mira Okafor via graph-curator");
    expect(rows, event.action).toContain("Mira Okafor via graph-curator");
    expect(rows.indexOf(proposer), event.action).toBeGreaterThan(rows.indexOf(label));
  }
  // 中文同样写出提议人
  expect(zh.kbset.auditProposedBy(zh.review.tokenVia("Mira Okafor", "graph-curator"))).toBe(
    "由 Mira Okafor 经 graph-curator 提议",
  );
  // 主人的账号已移除：照旧写那句「一位已移除的用户」
  const gone = answeredBy("agent.proposal.declined", { agent_proposal: proposal });
  delete gone.proposed_by_owner;
  expect(rowsOf(renderEvents([gone]))).toContain(
    S.kbset.auditProposedBy(`${S.kbset.deletedUser} via graph-curator`),
  );
  // 没有 proposed_by_token 的人的决定：照旧，没有提议人
  const own = answeredBy("violation.decided", { resolution: "accepted" }, "axiom_violation");
  delete own.proposed_by_token;
  delete own.proposed_by_owner;
  const rows = rowsOf(renderEvents([own]));
  expect(rows).not.toContain(" via ");
  expect(rows).not.toContain(S.kbset.auditProposedBy(""));
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
