import type { AgentProposal, MergeLog } from "../api";
import { S } from "../i18n";
import { Chip } from "../ui";
import {
  tokenActionLabel,
  tokenProposalLabel,
  tokenReasonLabel,
  viaToken,
} from "./TokenAgentRow";

/* ---------- 卡片上的令牌 agent（0076） ----------
   三样东西，都从服务端挂在卡片上的键来：
   - 一条开着的提议（`agent_proposal`）：重复项的卡片上挨着治理 agent 的那个标签，
     合并历史的一行上挨着「谁合的」。它只是告诉人「有一条等你答」——答在 Agent 一档
   - 一列开着的提议（`agent_proposals`）：Review 其余各档的卡片。一个目标上可以同时开着
     几条（同一条事实的确认与拒绝是两种操作），所以是一列，一条一个标签
   - 一次合并是谁做的（`agent`）：令牌 agent 做的合并写成「主人 via 令牌名」，
     不写成主人自己合的。合并日志的 `merged_by_name` 仍是主人，所以分辨只能靠这个键 */

/** 开着的提议：谁（主人 via 令牌名）想做什么、做哪一个动作；悬停是为什么没有直接落地。
 *  合并与撤回的提议没有动作，写出来与从前一样 */
export function AgentProposalChip({ proposal }: { proposal: AgentProposal }) {
  const what = tokenProposalLabel(proposal.operation);
  return (
    <Chip
      tone="warn"
      title={proposal.reason ? tokenReasonLabel(proposal.reason) : undefined}
    >
      {S.review.tokenProposalChip(
        viaToken(proposal.owner_name, proposal.token_name),
        proposal.action
          ? S.review.tokenWithAction(what, tokenActionLabel(proposal.operation, proposal.action))
          : what,
      )}
    </Chip>
  );
}

/** 一张卡片上开着的全部提议，一条一个标签，最早提的在前（服务端排好的次序）。
 *  **没有提议就什么都不渲染**——连包着的那一行也没有，卡片与从前一模一样。
 *  它只是告诉人「有一条等你答」：答在 Agent 一档，在卡片上直接裁是人自己的决定 */
export function AgentProposalChips({
  proposals,
}: {
  proposals?: AgentProposal[] | null;
}) {
  if (!proposals || proposals.length === 0) return null;
  return (
    <div className="mt-2 flex flex-wrap items-center gap-2">
      {proposals.map((p) => (
        <AgentProposalChip key={p.id} proposal={p} />
      ))}
    </div>
  );
}

/** 合并历史一行里「谁合的」那一句。令牌 agent 的合并：「by 主人 via 令牌名」；
 *  别的行与从前一样——人的名字，或者没有人（AI 裁决）。人批准提议做出的合并是人的合并，
 *  没有 `agent`，照人的写 */
export function MergeActor({
  merge,
}: {
  merge: Pick<MergeLog, "merged_by_name" | "agent">;
}) {
  if (merge.agent) {
    return (
      <>{S.review.mergedByToken(viaToken(merge.agent.owner_name, merge.agent.token_name))}</>
    );
  }
  return (
    <>{merge.merged_by_name ? S.review.mergedBy(merge.merged_by_name) : S.review.mergedByAi}</>
  );
}
