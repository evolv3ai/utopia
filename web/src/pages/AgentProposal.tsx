import type { AgentProposal, MergeLog } from "../api";
import { S } from "../i18n";
import { Chip } from "../ui";
import { tokenProposalLabel, tokenReasonLabel, viaToken } from "./TokenAgentRow";

/* ---------- 卡片上的令牌 agent（0076） ----------
   两样东西，都从服务端挂在卡片上的那两个键来：
   - 一条开着的提议（`agent_proposal`）：重复项的卡片上挨着治理 agent 的那个标签，
     合并历史的一行上挨着「谁合的」。它只是告诉人「有一条等你答」——答在 Agent 一档
   - 一次合并是谁做的（`agent`）：令牌 agent 做的合并写成「主人 via 令牌名」，
     不写成主人自己合的。合并日志的 `merged_by_name` 仍是主人，所以分辨只能靠这个键 */

/** 开着的提议：谁（主人 via 令牌名）想做什么；悬停是为什么没有直接落地 */
export function AgentProposalChip({ proposal }: { proposal: AgentProposal }) {
  return (
    <Chip
      tone="warn"
      title={proposal.reason ? tokenReasonLabel(proposal.reason) : undefined}
    >
      {S.review.tokenProposalChip(
        viaToken(proposal.owner_name, proposal.token_name),
        tokenProposalLabel(proposal.operation),
      )}
    </Chip>
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
