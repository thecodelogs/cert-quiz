-- Account status (suspend/reactivate from the users page) and a "last
-- active" readout, which is just the most recent session's creation time
-- kept on the user row instead of computed with a join on every page view.
alter table users
  add column status text not null default 'active' check (status in ('active', 'suspended')),
  add column last_active_at timestamptz;
