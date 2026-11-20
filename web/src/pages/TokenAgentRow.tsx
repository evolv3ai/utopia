import type { TokenAgentAction } from "../api";
import { S } from "../i18n";
import { Button, CARD_ACTIONS, Checkbox, Status, cn, localDate, type ChipTone } from "../ui";

/* ---------- 令牌 agent 的一笔（0076） ----------
   某个人的一枚令牌对 Review 做的动作。它不是治理 agent 的行，也不是人亲手做的：
   写成「主人 via 令牌名」，与点头卡片上 remember 的写法一致。下面几个纯函数
   Review 的 Agent 一档与库设置的 Activity 共用，两处说的是同一句话 */

/** 「主人 via 令牌名」。主人的名字读不到（账号已移除）时用现成的那句「一位已移除的用户」 */
export function viaToken(ownerName: string | null | undefined, tokenName: string): string {
  return S.review.tokenVia(ownerName || S.kbset.deletedUser, tokenName);
}

/** 令牌做的动作，写成一句话。**只给令牌的行用**：人和治理 agent 的行各有各的标签，
 *  不经过这张表。不认得的动作原样写出来，好过写错 */
export function tokenOperationLabel(operation: string): string {
  return S.review.tokenOperations[operation] ?? operation;
}

/** 同一个动作没落地、留成提议时的说法：它想做，还没做 */
export function tokenProposalLabel(operation: string): string {
  return S.review.tokenProposes[operation] ?? tokenOperationLabel(operation);
}

/** 一种操作要做的那个动作（0076）：键是「操作:动作」。不认得的原样写出来 */
export function tokenActionLabel(operation: string, action: string): string {
  return S.review.tokenActions[`${operation}:${action}`] ?? action;
}

/** 为什么没有直接落地。**认的是服务端的 code**，不是它那句英文说明 */
export function tokenReasonLabel(reason: string): string {
  return S.review.tokenReasons[reason] ?? reason;
}

/** 后来怎样了：approved / declined / superseded / undone */
export function tokenStateLabel(state: string): string {
  return S.review.tokenStates[state] ?? state;
}

/** 成批调用的号，写成一个短标签（「batch 1a2b3c4d」）。**取末八位**：号是 UUIDv7，开头是
 *  时间，前后脚的两批开头一样；末尾是随机的，才分得开。Agent 一档与 Activity 写法相同，
 *  同一批的几行靠它认到一起 */
export function batchLabel(batchId: string): string {
  return S.review.tokenBatch(batchId.replace(/-/g, "").slice(-8));
}

/** Activity 里一条事件的动作，在令牌这张表里有没有说法：令牌的行（带 `token_name`）都走
 *  这张表；人拒绝、或关掉一条过时提议的那两种事件是人的行，但动作是这一刀新写的，
 *  同样在这张表里。人批准令牌的提议、拒绝一条本体提案时写下的 `ontology_proposal.rejected`
 *  也是：这个动作只有答提议时才写（人自己在本体页拒绝不留审计行）。
 *  别的事件回 null，照旧走 Activity 自己的那张表 */
export function activityTokenLabel(action: string, byToken: boolean): string | null {
  if (byToken) return tokenOperationLabel(action);
  return action.startsWith("agent.proposal") || action === "ontology_proposal.rejected"
    ? tokenOperationLabel(action)
    : null;
}

/** 调用方自己报的 User-Agent。它由调用方完全控制，所以每次展示都写明是「declared」 */
export function declaredClientLabel(client: string): string {
  return S.kbset.auditDeclaredClient(client);
}

/** 还开着的提议：没落地（held / proposed），也还没人答 */
export function isOpenProposal(a: TokenAgentAction): boolean {
  return (a.outcome === "held" || a.outcome === "proposed") && !a.state;
}

const OUTCOME_TONE: Record<string, ChipTone> = {
  applied: "info",
  held: "warn",
  proposed: "warn",
};

const STATE_TONE: Record<string, ChipTone> = {
  approved: "success",
  declined: "neutral",
  superseded: "neutral",
  undone: "warn",
};

/** 手动合并与撤回是有方向的（源并进目标）：写成箭头。队列里的一对没有方向，方向到合并时才定 */
const DIRECTED = new Set(["merge.manual", "merge.revert"]);

/** Agent 一档里令牌的一行。
 *
 *  **按钮只在开着的提议上、并且调用方给了处理函数时才有**：落地的行没有可答的东西，
 *  Viewer 没有处理函数。答的是人——批准跑的是人自己的合并或撤回，记在这个人名下。
 *  `onPick` 给了，开着的提议左边多一个勾，跟着上面的「批准所选 / 拒绝所选」走 */
export function TokenAgentRow({
  a,
  busy,
  picked,
  onPick,
  onApprove,
  onDecline,
}: {
  a: TokenAgentAction;
  busy?: boolean;
  picked?: boolean;
  onPick?: (picked: boolean) => void;
  onApprove?: () => void;
  onDecline?: () => void;
}) {
  const proposal = a.outcome === "held" || a.outcome === "proposed";
  const open = isOpenProposal(a);
  const what = proposal ? tokenProposalLabel(a.operation) : tokenOperationLabel(a.operation);
  // 目标：不是一对的行（Review 其余各档）写服务端存下的那句话；一对的行照旧写两边的名字。
  // 两边都没有的行什么都不写——不拿问号去占一个本来就没有的位置
  const target = a.target_label
    ? a.target_label
    : a.left || a.right
      ? `${a.left ?? "?"} ${DIRECTED.has(a.operation) ? "→" : "≟"} ${a.right ?? "?"}`
      : null;
  // 批准了的短语 / 规则判定只是排下了投递任务：写「排着」，不写成已经落到图上
  const queued = a.state === "approved" && !!a.queued;
  return (
    <div className={cn("glass rounded-panel px-4 py-3", picked && "u-picked")}>
      <div className="flex items-center gap-3">
        {open && onPick && (
          <Checkbox
            className="shrink-0"
            checked={!!picked}
            disabled={busy}
            onChange={onPick}
            label={<span className="sr-only">{S.review.tokenPickProposal}</span>}
          />
        )}
        <span className="text-body text-ink shrink-0">
          {viaToken(a.owner_name, a.token_name)}
        </span>
        <span className="text-body text-ink-2 shrink-0">
          {a.action
            ? S.review.tokenWithAction(what, tokenActionLabel(a.operation, a.action))
            : what}
        </span>
        {target && <span className="text-body text-ink-2 truncate min-w-0">{target}</span>}
        <span className="ml-auto flex shrink-0 items-center gap-2">
          <Status tone={OUTCOME_TONE[a.outcome] ?? "neutral"}>
            {S.review.tokenOutcome[a.outcome] ?? a.outcome}
          </Status>
          {a.state && (
            <Status tone={queued ? "warn" : (STATE_TONE[a.state] ?? "neutral")}>
              {queued ? S.review.tokenQueued : tokenStateLabel(a.state)}
            </Status>
          )}
        </span>
      </div>
      <div className="mt-1 flex items-center gap-3 text-small text-ink-2">
        <span className="u-num text-fine shrink-0">{localDate(a.created_at)}</span>
        {/* 成批调用的一笔：同一批的几行写着同一个标签 */}
        {a.batch_id && (
          <span className="u-num text-fine shrink-0" title={a.batch_id}>
            {batchLabel(a.batch_id)}
          </span>
        )}
        {a.reason && <span className="shrink-0">{tokenReasonLabel(a.reason)}</span>}
        {a.rationale && <span className="truncate min-w-0">“{a.rationale}”</span>}
      </div>
      {open && (onApprove || onDecline) && (
        <div className={CARD_ACTIONS}>
          {onApprove && (
            <Button variant="primary" size="sm" disabled={busy} onClick={onApprove}>
              {S.review.tokenApprove}
            </Button>
          )}
          {onDecline && (
            <Button variant="secondary" size="sm" disabled={busy} onClick={onDecline}>
              {S.review.tokenDecline}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
