-- Whether a track is visible to learners at all (the Published/Draft toggle
-- on the admin dashboard's tracks table) -- distinct from an individual
-- question's status. Existing certs default to published so the learner
-- home page keeps showing exactly what it shows today.
alter table certifications
  add column published boolean not null default true;
