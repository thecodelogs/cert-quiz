-- pgcrypto: gen_random_uuid(), used by users.id and sessions.id
-- citext:   case-insensitive text, used by users.email so "A@x.com" and
--           "a@x.com" collide on the unique constraint
create extension if not exists pgcrypto;
create extension if not exists citext;
