-- Blog posts, written in the admin panel. The list at /blog is public; a
-- post's full body is only shown to logged-in users. `body` is Markdown,
-- rendered to HTML at request time (raw HTML in it is escaped, not trusted).
--
-- `published_at` is set the first time a post goes live and kept if it's
-- later unpublished and republished, so its date doesn't jump around.
create table posts (
    id           uuid primary key default uuid_v7(),
    slug         text not null unique,
    title        text not null,
    summary      text not null default '',
    body         text not null default '',
    status       text not null default 'draft' check (status in ('draft', 'published')),
    published_at timestamptz,
    author_id    uuid references users(id) on delete set null,
    created_at   timestamptz not null default now(),
    updated_at   timestamptz not null default now()
);

create index on posts (status, published_at desc);
