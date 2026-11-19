import type { TokenAgentAction } from "../api";
import { S } from "../i18n";
import { Status, localDate, type ChipTone } from "../ui";

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

/** 调用方自己报的 User-Agent。它由调用方完全控制，所以每次展示都写明是「declared」 */
export function declaredClientLabel(client: string): string {
  return S.kbset.auditDeclaredClient(client);
}

const OUTCOME_TONE: Record<string, ChipTone> = {
  applied: "info",
  held: "warn",
  proposed: "warn",
};

/** Agent 一档里令牌的一行。**没有按钮**：回答路由是给治理 agent 的决定用的 */
export function TokenAgentRow({ a }: { a: TokenAgentAction }) {
  return (
    <div className="glass rounded-panel px-4 py-3">
      <div className="flex items-center gap-3">
        <span className="text-body text-ink shrink-0">
          {viaToken(a.owner_name, a.token_name)}
        </span>
        <span className="text-body text-ink-2 shrink-0">{tokenOperationLabel(a.operation)}</span>
        <span className="text-body text-ink-2 truncate min-w-0">
          {a.left ?? "?"} ≟ {a.right ?? "?"}
        </span>
        <Status tone={OUTCOME_TONE[a.outcome] ?? "neutral"} className="ml-auto shrink-0">
          {S.review.tokenOutcome[a.outcome] ?? a.outcome}
        </Status>
      </div>
      <div className="mt-1 flex items-center gap-3 text-small text-ink-2">
        <span className="u-num text-fine shrink-0">{localDate(a.created_at)}</span>
        {a.rationale && <span className="truncate min-w-0">“{a.rationale}”</span>}
      </div>
    </div>
  );
}
