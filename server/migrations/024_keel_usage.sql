-- Anonymous Keel reports are separate from Oya Browser installs. The empty
-- day is the first-run marker sent before Keel has any completed usage day.
create table if not exists oya_browser.keel_usage (
  install_id text not null,
  day text not null,
  version text not null,
  os text not null,
  steps bigint not null check (steps >= 0 and steps <= 4294967295),
  license_id text,
  received_at timestamptz not null default now(),
  primary key (install_id, day)
);

grant all on oya_browser.keel_usage to service_role;
alter table oya_browser.keel_usage enable row level security;
