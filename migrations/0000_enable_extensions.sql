-- pgcrypto: gen_random_uuid(), used by sessions.id and by uuid_v7() below
-- citext:   case-insensitive text, used by users.email so "A@x.com" and
--           "a@x.com" collide on the unique constraint
create extension if not exists pgcrypto;
create extension if not exists citext;

-- Every table's id is a UUID v7 (RFC 9562): a 48-bit millisecond timestamp,
-- then random bits, so ids sort by creation time and index almost as well as
-- a serial. Defined here rather than using Postgres 18's built-in uuidv7()
-- so it works on any version.
--
-- The 12 bits after the version come from a cycling counter instead of
-- being random, so ids made in the same millisecond (a multi-row seed
-- insert, say) still sort in the order they were made.
--
-- sessions.id is the exception and stays a fully random v4: it is the login
-- cookie, and a v7 would carry fewer random bits and reveal when it was made.
create sequence uuid_v7_seq minvalue 0 maxvalue 4095 start 0 cycle;

create function uuid_v7() returns uuid
language sql volatile as $$
    select encode(
        overlay(
            overlay(uuid_send(gen_random_uuid())
                    placing substring(int8send(floor(extract(epoch from clock_timestamp()) * 1000)::bigint) from 3)
                    from 1 for 6)
            placing int2send((28672 + nextval('uuid_v7_seq'))::smallint)  -- 0x7000 | counter
            from 7 for 2),
        'hex')::uuid
$$;
