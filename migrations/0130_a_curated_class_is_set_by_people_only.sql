-- 0077：人管的类（#8）。
--
-- 一个类的描述挡不住泛词：`concept` 写明了「不收一般抽象名词」，类别词对齐还是在
-- 36 小时里把一万多个泛词（approach、method、problem……）绑了上去。描述是给模型看的
-- 建议，这一列是规则：
--
--   curated = true  类别词对齐不把它放进候选，代理的判定不再绑到它；抽取按绑定给类时
--                   跳过代理绑到它的类别词。只有人改实体的类、人判的类别词、导入，
--                   才会让一个实体落进这个类
--
-- 改这一列会动 updated_at：候选变了，指纹对不上，代理以前绑到它的类别词下一轮重判。
--
-- 可重放，理由同 0128。

ALTER TABLE entity_types ADD COLUMN IF NOT EXISTS curated BOOLEAN NOT NULL DEFAULT false;
