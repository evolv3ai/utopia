-- Seeds one existing knowledge base so that every Review section lists something
-- (ADR 0076: the token read surface is checked against a base with items in all
-- thirteen sections).
--
-- Usage, on a migrated database, for a base that already exists:
--
--     psql "$UTOPIA_DATABASE_URL" -v ON_ERROR_STOP=1 -1 \
--          -c "SET seed.kb = '<knowledge base uuid>'" -f scripts/seed-review-sections.sql
--
-- The base is read from the session setting `seed.kb`; no psql variable is used, so
-- the same file runs unchanged from a driver on one connection (the server tests do
-- that). Direct inserts only: no model job, no worker.
--
-- What it guarantees:
--   * every row carries the given base's `kb_id`; no other base is read or written;
--   * ids are derived from the base id (md5 of `<kb>:<tag>`), keys and names carry a
--     `seed` prefix, and every insert is `ON CONFLICT DO NOTHING` — running it again,
--     or on a base that already has rows, adds nothing twice and changes no row;
--   * no DDL and no update or delete.
--
-- After it runs the base holds:
--   pending 2 · duplicates 4 (three same-type, one type conflict; the one with reason
--   `seed: token keep` is left for a token to keep) · conflicts 2 · unconfirmed 2 ·
--   lowconf 3 (one derived) · mappings 1 · alignment 2 phrases + 1 kind word + 2 rules
--   (a rule is last) · errata 2 · violations 2 (one `derived_contradiction` with a
--   two-step proof, one `asymmetry`) · defects 2 · merges 2 · agent 2 governor rows
--   (one open proposal) · decisions 2 events.
-- The token-agent row of Agent History cannot be made by an insert alone (it needs an
-- issued token); the test fixture adds it through `token_agent_actions::keep_duplicate_pair`.

-- ---------------------------------------------------------------------------
-- Ontology: two classes, four properties
-- ---------------------------------------------------------------------------
INSERT INTO entity_types (id, kb_id, key, label)
SELECT md5(k.p || x.tag)::uuid, k.kb, x.key, x.label
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES ('type-person', 'seed_person', 'Seed person'),
               ('type-org',    'seed_org',    'Seed organization')) AS x(tag, key, label)
ON CONFLICT DO NOTHING;

INSERT INTO relation_types (id, kb_id, key, label, temporal, kind, datatype,
                            functional, is_asymmetric, is_symmetric, is_transitive)
SELECT md5(k.p || x.tag)::uuid, k.kb, x.key, x.label, x.temporal, x.kind, x.datatype,
       x.is_func, x.is_asym, x.is_sym, x.is_trans
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('rel-works',   'seed_works_at', 'seed works at', 'state',   'relation',  NULL,   false, false, false, false),
         ('rel-manages', 'seed_manages',  'seed manages',  'state',   'relation',  NULL,   false, true,  true,  false),
         ('rel-part',    'seed_part_of',  'seed part of',  'eternal', 'relation',  NULL,   false, true,  false, true),
         ('rel-lives',   'seed_lives_in', 'seed lives in', 'state',   'attribute', 'text', true,  false, false, false)
       ) AS x(tag, key, label, temporal, kind, datatype, is_func, is_asym, is_sym, is_trans)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Entities. Pairs that share a name are the duplicate candidates; `ent-m*-source`
-- are the merged-away sides of the two merge log rows.
-- ---------------------------------------------------------------------------
INSERT INTO entities (id, kb_id, type_id, canonical_name, specific_type, merged_into)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || x.type_tag)::uuid, x.name, x.specific_type,
       md5(k.p || x.merged_into)::uuid
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         -- merge targets first: a merged-away row points at its target
         ('ent-m1-target', 'type-person', 'Seed Hana Ito',        NULL,           NULL),
         ('ent-m2-target', 'type-org',    'Seed Initrode',        NULL,           NULL),
         ('ent-m1-source', 'type-person', 'Seed H. Ito',          NULL,           'ent-m1-target'),
         ('ent-m2-source', 'type-org',    'Seed Initrode Inc',    NULL,           'ent-m2-target'),
         ('ent-ada',       'type-person', 'Seed Ada Quill',       NULL,           NULL),
         ('ent-bo',        'type-person', 'Seed Bo Marsh',        NULL,           NULL),
         ('ent-cy',        'type-person', 'Seed Cy Okoro',        NULL,           NULL),
         ('ent-acme',      'type-org',    'Seed Acme',            NULL,           NULL),
         ('ent-globex',    'type-org',    'Seed Globex',          NULL,           NULL),
         ('ent-initech',   'type-org',    'Seed Initech',         'Seed Startup', NULL),
         ('ent-d1-left',   'type-person', 'Seed Dana Li',         NULL,           NULL),
         ('ent-d1-right',  'type-person', 'Seed Dana Li',         NULL,           NULL),
         ('ent-d2-left',   'type-person', 'Seed Eli Park',        NULL,           NULL),
         ('ent-d2-right',  'type-person', 'Seed Eli Park',        NULL,           NULL),
         ('ent-d3-left',   'type-person', 'Seed Mercury',         NULL,           NULL),
         ('ent-d3-right',  'type-org',    'Seed Mercury',         NULL,           NULL),
         ('ent-d4-left',   'type-person', 'Seed Finn Roe',        NULL,           NULL),
         ('ent-d4-right',  'type-person', 'Seed Finn Roe',        NULL,           NULL),
         ('ent-d5-left',   'type-person', 'Seed Gia Moss',        NULL,           NULL),
         ('ent-d5-right',  'type-person', 'Seed Gia Moss',        NULL,           NULL),
         ('ent-concept',   NULL,          'Seed monthly revenue', NULL,           NULL)
       ) AS x(tag, type_tag, name, specific_type, merged_into)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- One document with a current passage and one a newer version replaced
-- ---------------------------------------------------------------------------
INSERT INTO documents (id, kb_id, filename, mime, sha256, status, graph_status, chunk_count)
SELECT md5(k.p || 'doc')::uuid, k.kb, 'seed-review-sections.md', 'text/markdown',
       md5(k.p || 'doc-sha'), 'ready', 'done', 2
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k
ON CONFLICT DO NOTHING;

INSERT INTO chunks (id, kb_id, document_id, seq, text, char_start, char_end,
                    superseded_at, extracted_at)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || 'doc')::uuid, x.seq, x.text, 0, length(x.text),
       CASE WHEN x.superseded THEN now() END, now()
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('chunk-live', 0, false,
          'Seed Ada Quill moved to Seed Globex and advises Seed Bo Marsh. She lives in Lisbon.'),
         ('chunk-old',  1, true,
          'Seed Cy Okoro works at Seed Acme and manages Seed Ada Quill.')
       ) AS x(tag, seq, superseded, text)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Awaiting your nod (`pending`): two statements a person has not confirmed
-- ---------------------------------------------------------------------------
INSERT INTO pending_facts (id, kb_id, subject_id, predicate_id, object_id, proposed_predicate,
                           confidence, chunk_id, phrase)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || x.subject)::uuid, md5(k.p || x.predicate)::uuid,
       md5(k.p || x.object)::uuid, x.proposed_predicate, x.confidence,
       md5(k.p || 'chunk-live')::uuid, x.phrase
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('pending-1', 'ent-ada', 'rel-works', 'ent-globex', NULL,      0.6::real, 'moved to'),
         ('pending-2', 'ent-ada', NULL,        'ent-bo',     'advises', 0.5::real, 'advises')
       ) AS x(tag, subject, predicate, object, proposed_predicate, confidence, phrase)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Facts behind Conflicts, Unconfirmed, Low confidence and Axioms
-- ---------------------------------------------------------------------------
INSERT INTO facts (id, kb_id, subject_id, predicate_id, object_id, object_value, confidence,
                   derived_by_rule)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || x.subject)::uuid, md5(k.p || x.predicate)::uuid,
       md5(k.p || x.object)::uuid,
       CASE WHEN x.value IS NOT NULL
            THEN jsonb_build_object('value', x.value, 'summary', x.value) END,
       x.confidence,
       CASE WHEN x.derived THEN md5(k.p || 'derived-by-rule')::uuid END
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         -- Conflicts: one person, two employers, no dates on either
         ('fact-conflict-1-old', 'ent-ada',     'rel-works',   'ent-acme',    NULL,     1.0::real, false),
         ('fact-conflict-1-new', 'ent-ada',     'rel-works',   'ent-globex',  NULL,     1.0::real, false),
         ('fact-conflict-2-old', 'ent-bo',      'rel-works',   'ent-acme',    NULL,     1.0::real, false),
         ('fact-conflict-2-new', 'ent-bo',      'rel-works',   'ent-initech', NULL,     1.0::real, false),
         -- Unconfirmed: their only evidence sits in the replaced passage
         ('fact-stale-1',        'ent-cy',      'rel-works',   'ent-acme',    NULL,     1.0::real, false),
         ('fact-stale-2',        'ent-cy',      'rel-manages', 'ent-ada',     NULL,     1.0::real, false),
         -- Low confidence: two asserted, one a rule derived
         ('fact-low-1',          'ent-ada',     'rel-lives',   NULL,          'Lisbon', 0.4::real, false),
         ('fact-low-2',          'ent-bo',      'rel-lives',   NULL,          'Porto',  0.5::real, false),
         ('fact-low-derived',    'ent-cy',      'rel-lives',   NULL,          'Faro',   0.6::real, true),
         -- Axioms: a two-way `manages`, and a `part of` chain whose conclusion an assertion denies
         ('fact-asym-1',         'ent-ada',     'rel-manages', 'ent-bo',      NULL,     1.0::real, false),
         ('fact-asym-2',         'ent-bo',      'rel-manages', 'ent-ada',     NULL,     1.0::real, false),
         ('fact-part-1',         'ent-acme',    'rel-part',    'ent-globex',  NULL,     1.0::real, false),
         ('fact-part-2',         'ent-globex',  'rel-part',    'ent-initech', NULL,     1.0::real, false),
         ('fact-part-against',   'ent-initech', 'rel-part',    'ent-acme',    NULL,     1.0::real, false)
       ) AS x(tag, subject, predicate, object, value, confidence, derived)
ON CONFLICT DO NOTHING;

INSERT INTO fact_evidence (fact_id, chunk_id, quote, document_id, doc_version)
SELECT md5(k.p || x.fact)::uuid, md5(k.p || x.chunk)::uuid, x.quote, md5(k.p || 'doc')::uuid, 1
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('fact-stale-1',        'chunk-old',  'Seed Cy Okoro works at Seed Acme'),
         ('fact-stale-2',        'chunk-old',  'manages Seed Ada Quill'),
         ('fact-low-1',          'chunk-live', 'She lives in Lisbon.'),
         ('fact-conflict-1-new', 'chunk-live', 'Seed Ada Quill moved to Seed Globex'),
         ('fact-part-1',         'chunk-live', NULL),
         ('fact-part-2',         'chunk-live', NULL)
       ) AS x(fact, chunk, quote)
ON CONFLICT DO NOTHING;

INSERT INTO fact_conflicts (id, kb_id, old_fact_id, new_fact_id, reason)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || x.old_fact)::uuid, md5(k.p || x.new_fact)::uuid,
       'no_time'
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES ('conflict-1', 'fact-conflict-1-old', 'fact-conflict-1-new'),
               ('conflict-2', 'fact-conflict-2-old', 'fact-conflict-2-new')
       ) AS x(tag, old_fact, new_fact)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Axioms (`violations`). The `derived_contradiction` row is shaped as the checker
-- writes it: `left_fact` is the assertion the conclusion runs into, `right_fact` the
-- last premise, `path` the asserted premises in order — the proof read expands `path`.
-- ---------------------------------------------------------------------------
INSERT INTO axiom_violations (id, kb_id, kind, left_fact, right_fact, path, detail)
SELECT md5(k.p || 'violation-asymmetry')::uuid, k.kb, 'asymmetry',
       md5(k.p || 'fact-asym-1')::uuid, md5(k.p || 'fact-asym-2')::uuid, '{}'::uuid[], '{}'::jsonb
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k
ON CONFLICT DO NOTHING;

INSERT INTO axiom_violations (id, kb_id, kind, left_fact, right_fact, path, detail)
SELECT md5(k.p || 'violation-derived')::uuid, k.kb, 'derived_contradiction',
       md5(k.p || 'fact-part-against')::uuid, md5(k.p || 'fact-part-2')::uuid,
       ARRAY[md5(k.p || 'fact-part-1')::uuid, md5(k.p || 'fact-part-2')::uuid],
       jsonb_build_object(
           'axiom', 'asymmetric',
           'rule', 'transitive',
           'attribute_rule_id', NULL,
           'via', md5(k.p || 'rel-part')::uuid,
           'via_label', 'seed part of',
           'subject_id', md5(k.p || 'ent-acme')::uuid,
           'subject', 'Seed Acme',
           'predicate_id', md5(k.p || 'rel-part')::uuid,
           'predicate', 'seed part of',
           'object_id', md5(k.p || 'ent-initech')::uuid,
           'object', 'Seed Initech',
           'valid_from', NULL,
           'valid_to', NULL,
           'premises', jsonb_build_array(md5(k.p || 'fact-part-1')::uuid,
                                         md5(k.p || 'fact-part-2')::uuid))
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Ontology (`defects`)
-- ---------------------------------------------------------------------------
INSERT INTO ontology_defects (id, kb_id, kind, subject, detail)
SELECT md5(k.p || x.tag)::uuid, k.kb, x.kind, md5(k.p || x.subject)::uuid, '{}'::jsonb
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES ('defect-1', 'symmetric_and_asymmetric', 'rel-manages'),
               ('defect-2', 'inverse_of_itself',        'rel-part')
       ) AS x(tag, kind, subject)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Duplicates. Four pairs wait; the fifth was already kept apart by the governor.
-- ---------------------------------------------------------------------------
INSERT INTO resolution_reviews (id, kb_id, left_id, right_id, score, reason, stage, status,
                                created_at, decided_at)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || x.left_tag)::uuid, md5(k.p || x.right_tag)::uuid,
       0.5, x.reason, 'human', x.status, now() - x.age, CASE WHEN x.status <> 'pending' THEN now() END
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('review-1', 'ent-d1-left', 'ent-d1-right', 'namesake',         'pending', interval '5 minutes'),
         ('review-2', 'ent-d2-left', 'ent-d2-right', 'namesake',         'pending', interval '4 minutes'),
         ('review-3', 'ent-d3-left', 'ent-d3-right', 'namesake',         'pending', interval '3 minutes'),
         ('review-4', 'ent-d4-left', 'ent-d4-right', 'seed: token keep', 'pending', interval '2 minutes'),
         ('review-5', 'ent-d5-left', 'ent-d5-right', 'namesake',         'kept',    interval '6 minutes')
       ) AS x(tag, left_tag, right_tag, reason, status, age)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Merges: the merge log
-- ---------------------------------------------------------------------------
INSERT INTO entity_merges (id, kb_id, source_id, target_id, reason, created_at)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || x.source)::uuid, md5(k.p || x.target)::uuid,
       x.reason, now() - x.age
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES ('merge-1', 'ent-m1-source', 'ent-m1-target', 'seed: same person', interval '8 minutes'),
               ('merge-2', 'ent-m2-source', 'ent-m2-target', 'seed: same company', interval '7 minutes')
       ) AS x(tag, source, target, reason, age)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Agent History, governor side: one open proposal and one applied keep
-- ---------------------------------------------------------------------------
INSERT INTO agent_decisions (id, kb_id, run_id, target_kind, target_id, action, confidence,
                             reason, status, created_at, decided_at)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || 'govern-run')::uuid, 'review',
       md5(k.p || x.review)::uuid, x.action, x.confidence, x.reason, x.status, now() - x.age,
       CASE WHEN x.status <> 'proposed' THEN now() - x.age END
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('agent-1', 'review-1', 'merge', 0.8::real, 'seed: same name, same class', 'proposed', interval '1 minute'),
         ('agent-2', 'review-5', 'keep',  0.9::real, 'seed: different people',      'applied',  interval '6 minutes')
       ) AS x(tag, review, action, confidence, reason, status, age)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Semantic-layer mapping (`mappings`, a session-only list queue)
-- ---------------------------------------------------------------------------
INSERT INTO concept_mappings (id, kb_id, concept_id, source, table_name, expr, unit, summary, status)
SELECT md5(k.p || 'mapping-1')::uuid, k.kb, md5(k.p || 'ent-concept')::uuid, 'seed_warehouse',
       'seed_orders', 'sum(amount)', 'USD', 'seed: revenue summed per month', 'proposed'
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Alignment: two phrase signatures, one kind word, two proposed implication rules.
-- `decided_at` is explicit: the list is ordered by it, and the second rule is the
-- last item — the one a pager using the badge count never reaches.
-- ---------------------------------------------------------------------------
INSERT INTO phrase_bindings (id, kb_id, phrase, subject_type_id, object_type_id, status, votes,
                             statement_count, examples, decided_at)
SELECT md5(k.p || x.tag)::uuid, k.kb, x.phrase, md5(k.p || 'type-person')::uuid,
       md5(k.p || x.object_type)::uuid, 'undecided',
       '{"first": null, "second": null}'::jsonb, 1, ARRAY[x.example], now() - x.age
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('phrase-1', 'seed advises',     'type-person', 'Seed Ada Quill —seed advises→ Seed Bo Marsh', interval '5 hours'),
         ('phrase-2', 'seed is based in', 'type-org',    'Seed Cy Okoro —seed is based in→ Seed Acme',  interval '4 hours')
       ) AS x(tag, phrase, object_type, example, age)
ON CONFLICT DO NOTHING;

INSERT INTO type_bindings (id, kb_id, kind_word, words, status, votes, decided_at)
SELECT md5(k.p || 'kind-word-1')::uuid, k.kb, 'seed startup', ARRAY['Seed Startup'], 'undecided',
       '{"first": null, "second": null}'::jsonb, now() - interval '3 hours'
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k
ON CONFLICT DO NOTHING;

INSERT INTO implication_rules (id, kb_id, trigger, phrase, subject_type_id, object_type_id,
                               conclude_property_id, status, votes, statement_count, examples,
                               decided_at)
SELECT md5(k.p || x.tag)::uuid, k.kb, x.trigger, x.phrase, md5(k.p || x.subject_type)::uuid,
       md5(k.p || x.object_type)::uuid, md5(k.p || 'rel-works')::uuid, 'proposed',
       '{"first": "approve", "second": "reject"}'::jsonb, 1, ARRAY[x.example], now() - x.age
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('rule-1', 'phrase',    'seed joined',  'type-person', 'type-org',
          'Seed Bo Marsh —seed joined→ Seed Initech', interval '2 hours'),
         ('rule-2', 'kind_word', 'seed startup', NULL,          NULL,
          'Seed Initech',                             interval '1 hour')
       ) AS x(tag, trigger, phrase, subject_type, object_type, example, age)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Errata: two actions the gate held for a person
-- ---------------------------------------------------------------------------
INSERT INTO errata_runs (id, kb_id, document_id, flagged, sampled, finished_at)
SELECT md5(k.p || 'errata-run')::uuid, k.kb, md5(k.p || 'doc')::uuid, 2, 2, now()
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k
ON CONFLICT DO NOTHING;

INSERT INTO errata_actions (id, kb_id, run_id, document_id, fact_id, predicate_id, flag, action,
                            reason, quote, proposed, status, detail, created_at)
SELECT md5(k.p || x.tag)::uuid, k.kb, md5(k.p || 'errata-run')::uuid, md5(k.p || 'doc')::uuid,
       md5(k.p || x.fact)::uuid, md5(k.p || 'rel-lives')::uuid, x.flag, x.action, x.reason,
       'She lives in Lisbon.',
       jsonb_build_object(
           'subject_id', md5(k.p || x.subject)::uuid,
           'predicate_id', md5(k.p || 'rel-lives')::uuid,
           'object_value', jsonb_build_object('value', x.value, 'summary', x.value),
           'subject', x.subject_name,
           'property', 'seed_lives_in',
           'object', x.value),
       'held', 'seed: held by the gate', now() - x.age
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('errata-1', 'fact-low-1', 'ent-ada', 'Seed Ada Quill', 'Lisbon', 'no_date',     'revise',
          'seed: the passage gives no date for this', interval '10 minutes'),
         ('errata-2', 'fact-low-2', 'ent-bo',  'Seed Bo Marsh',  'Porto',  'name_absent', 'retract',
          'seed: the passage never names this person', interval '9 minutes')
       ) AS x(tag, fact, subject, subject_name, value, flag, action, reason, age)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Decisions: two events of the review families (`review.` and `fact.`)
-- ---------------------------------------------------------------------------
INSERT INTO audit_events (id, kb_id, action, target_kind, target_id, detail, created_at, actor_label)
SELECT md5(k.p || x.tag)::uuid, k.kb, x.action, x.target_kind, md5(k.p || x.target)::uuid,
       x.detail::jsonb, now() - x.age, 'seed'
  FROM (SELECT current_setting('seed.kb')::uuid AS kb, current_setting('seed.kb') || ':' AS p) k,
       (VALUES
         ('audit-1', 'review.keep',  'review', 'review-5',
          '{"left": "Seed Gia Moss", "right": "Seed Gia Moss", "score": 0.5, "why": "seed: different people"}',
          interval '6 minutes'),
         ('audit-2', 'fact.confirm', 'fact',   'fact-part-1',
          '{"subject": "Seed Acme", "predicate": "seed part of", "object": "Seed Globex", "confidence": 1.0}',
          interval '5 minutes')
       ) AS x(tag, action, target_kind, target, detail, age)
ON CONFLICT DO NOTHING;
