-- Admin-panel concepts that don't exist yet: a question can be a draft
-- (not shown to learners), retired (pulled from sets but kept for history),
-- has its own difficulty (distinct from certifications.difficulty, which is
-- the exam's overall level), and optional tags for search.
--
-- Existing rows default to 'live' / 'Medium' so the 60 already-seeded
-- questions keep behaving exactly as they do today.
alter table questions
  add column status text not null default 'live' check (status in ('draft', 'live', 'retired')),
  add column difficulty text not null default 'Medium' check (difficulty in ('Easy', 'Medium', 'Hard')),
  add column tags text[] not null default '{}';

create index on questions (certification_id, status);
