-- 0076：令牌 agent 的台账长出它这一刀要写的列——闸门的理由、批次、合并、后来的状态。
--
-- 0128 只建了「分开一对」会写的列，并且说了：其余的列各自跟着第一次写它的那一刀来。
-- 这一刀让写令牌合并、撤回合并、成批裁决，每一笔都先过 0027 的闸门；被闸门留下的、
-- 或者只能提议的调用，不动图，作为一条**提议**留在这张台账里等人答。所以台账要记得住：
--
--   gate_reason  为什么没有直接落地。闸门留下的（outcome = held）：identity、
--                contradiction、derived、answered、kept_by_person、reverted_merge；
--                只能提议的（outcome = proposed）：merge_by_person、merge_by_governor、
--                breaker。直接落地的行这一列是 NULL
--   batch_id     这一笔属于哪一次成批调用（服务端发的号）；单笔调用是 NULL
--   merge_id     落地的合并行：它做出的那次合并；merge.revert 行：它要撤的那次合并；
--                被人批准的合并提议：批准时做出的那次合并。裸 UUID，没有外键——
--                台账要活得比它指着的东西久，理由与 0128 的 owner_id / token_id 相同
--   target_key   「同一个目标」的文本钥匙：队列合并是审核行 id，手动合并是两个实体 id
--                按升序用 | 连起来（不分方向的一对），撤回是合并 id
--   state        提议后来怎样了：approved、declined、superseded。`undone` 不存——
--                一次合并被撤回，读的时候从 `entity_merges.reverted_at` 看出来，
--                这样人照旧从合并历史撤回时一行都不用多写
--   state_at     什么时候答的
--   state_by     谁答的（裸 UUID，理由同上）
--
-- **一个库、一种操作、一个目标，同一时刻只有一条开着的提议**（outcome 是 held 或
-- proposed、state 还空着）。重复调用、同时到的两个调用，由下面那个部分唯一索引裁决：
-- 后到的写不进去，读到先到的那一行。
--
-- 全部写成可重放的，理由同 0128：改了编号的迁移会在跑过旧号的库上再执行一遍，
-- 那一遍必须是空操作。

ALTER TABLE token_agent_actions ADD COLUMN IF NOT EXISTS gate_reason TEXT;
ALTER TABLE token_agent_actions ADD COLUMN IF NOT EXISTS batch_id UUID;
ALTER TABLE token_agent_actions ADD COLUMN IF NOT EXISTS merge_id UUID;
ALTER TABLE token_agent_actions ADD COLUMN IF NOT EXISTS target_key TEXT;
ALTER TABLE token_agent_actions ADD COLUMN IF NOT EXISTS state TEXT;
ALTER TABLE token_agent_actions ADD COLUMN IF NOT EXISTS state_at TIMESTAMPTZ;
ALTER TABLE token_agent_actions ADD COLUMN IF NOT EXISTS state_by UUID;

-- CHECK 约束没有 IF NOT EXISTS，所以先看在不在。按表的 oid 找而不是按名字拼 schema：
-- 迁移在哪个 search_path 下跑都指着同一张表
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'token_agent_actions_state_check'
           AND conrelid = 'token_agent_actions'::regclass
    ) THEN
        ALTER TABLE token_agent_actions
            ADD CONSTRAINT token_agent_actions_state_check
            CHECK (state IN ('approved', 'declined', 'superseded', 'undone'));
    END IF;
END
$$;

-- 一个目标一条开着的提议。target_key 为 NULL 的行（0128 写下的分开）不参与：
-- 唯一索引里 NULL 互不相等，而且它们的 outcome 是 applied，本就不在这个部分索引里
CREATE UNIQUE INDEX IF NOT EXISTS token_agent_actions_open_proposal_idx
    ON token_agent_actions (kb_id, operation, target_key)
    WHERE outcome IN ('held', 'proposed') AND state IS NULL;

-- 「这次合并是不是令牌 agent 做的、是不是令牌撤的」按 merge_id 查：撤回之前要问，
-- 治理 agent 数类型对的撤回时也要问
CREATE INDEX IF NOT EXISTS token_agent_actions_merge_idx
    ON token_agent_actions (merge_id) WHERE merge_id IS NOT NULL;
