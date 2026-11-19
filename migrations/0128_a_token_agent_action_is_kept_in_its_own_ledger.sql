-- 0076：令牌 agent 在 Review 里做的每一个动作，在自己的台账里留一行。
--
-- 令牌以发它的人的身份行事（0014 决定 1），所以令牌裁的一对存下来和这个人亲手裁的
-- 一模一样——治理 agent 读先例时会把它当成人的决定，越读越自信。这里把两件事分开：
--
--   1. `token_agent_actions`：令牌 agent 的台账。一行 = 一次动作：哪个库、什么操作、
--      对着什么、谁的令牌、哪一枚、当时叫什么名字、服务端的策略给了什么结果
--      （applied / held / proposed）、请求里带了什么。
--   2. `resolution_reviews.decided_token`：这一对是经哪一枚令牌裁的。人亲手裁的、
--      机器自己裁的，这一列都是 NULL；先例查询据此把令牌裁的排除在「人的决定」之外。
--
-- 这一刀只建它自己会写的列（0076 决定 7）。后续状态、闸门理由、批次号、撤销引用、
-- 回答的人，各自跟着第一次写它的那一刀来。
--
-- 全部写成可重放的（IF NOT EXISTS）：向上游提交时编号可能要改，改了号的迁移会在
-- 跑过旧号的库上再执行一遍，那一遍必须是空操作。

CREATE TABLE IF NOT EXISTS token_agent_actions (
    id          UUID PRIMARY KEY,
    -- 与 `agent_decisions` 同一个做法：行属于库，库删了行跟着走
    kb_id       UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    -- 操作的稳定名字，与审计动作同名：`review.keep`
    operation   TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id   UUID NOT NULL,
    -- **裸 UUID，没有外键**，与 `audit_events.actor_id` 同理：台账要活得比人和钥匙久。
    -- `personal_tokens.user_id` 是级联的——人没了钥匙跟着没；这里若也挂外键，
    -- 「这枚令牌做过什么」就会在最需要查的时候一起消失
    owner_id    UUID NOT NULL,
    token_id    UUID NOT NULL,
    -- 动作发生那一刻令牌的名字。令牌可以改名、撤销、删除，这一行仍读作当时的样子
    token_name  TEXT NOT NULL,
    outcome     TEXT NOT NULL CHECK (outcome IN ('applied', 'held', 'proposed')),
    -- 请求带来的东西，按 operation 语义自定；keep 记 {left, right, score, why}
    payload     JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 一个库的台账，最新的在前：Agent History 就是这么列的
CREATE INDEX IF NOT EXISTS token_agent_actions_kb_idx
    ON token_agent_actions (kb_id, created_at DESC, id DESC);

-- 裸 UUID，理由同上：令牌删了，「这一对是令牌裁的」这件事不能变回「人裁的」
ALTER TABLE resolution_reviews ADD COLUMN IF NOT EXISTS decided_token UUID;
