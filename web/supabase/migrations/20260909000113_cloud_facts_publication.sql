-- Cloud facts publication storage (facts/v1).
--
-- This is the missing DDL for storage that three existing contracts already
-- assume; it introduces no new catalog, publication service, provider, or
-- execution loop:
--
--   * web/scripts/facts-publish.mjs — writes public.facts_key and
--     public.facts_release with the service-role key, looks channels up by
--     (scope = 'global', slug), reads back id/payload_sha256, and revokes by
--     (channel_id, facts_version).
--   * web/lib/cloud-facts.ts — reads exactly one row of public.facts_current
--     over PostgREST with the publishable key:
--     channel=eq.<slug>&scope=eq.global&limit=1, selecting channel,
--     release_id, facts_version, schema_version, envelope_version, applies_to,
--     key_id, payload_b64, sig_b64, sigs, payload_sha256, published_at,
--     not_after.
--   * docs/CLOUD_FACTS.md — "facts_current must be a read-only view with
--     explicit SELECT grants, RLS and policies limited to published public
--     channels."
--
-- Trust model: the database is an untrusted transport. Signatures are verified
-- by clients against keys pinned in the binary (crates/config/src/cloud_facts/
-- keys.rs) and in web/lib/cloud-facts/keys.ts. public.facts_key is an operator
-- registry, never a trust root, so it is not exposed to anon/authenticated at
-- all and no row here can make a fixture key trusted.
--
-- Rollback: public.facts_channel.max_facts_version is a high-water mark that
-- never decreases. facts_current serves the head version only, so revoking the
-- head makes the channel serve nothing (HTTP 404 "no-facts") instead of
-- silently re-serving an older accepted version. The fix for a bad release is
-- publishing a higher facts_version; clients enforce their own version floor
-- as well.
--
-- Re-applying this file is safe (no statement errors on a database that
-- already has these objects), but it does NOT reconcile a pre-existing table
-- whose columns or constraints differ: compare against the live schema before
-- applying to a project that already carries these tables.

-- ---------------------------------------------------------------------------
-- Tables
-- ---------------------------------------------------------------------------

create table if not exists public.facts_channel (
  id uuid primary key default gen_random_uuid(),
  scope text not null default 'global'
    constraint facts_channel_scope_check check (scope ~ '^[a-z0-9][a-z0-9-]{0,31}$'),
  slug text not null
    constraint facts_channel_slug_check check (slug ~ '^[a-z0-9][a-z0-9-]{0,31}$'),
  -- Fail closed: a new channel is invisible to anon until it is deliberately
  -- made public.
  visibility text not null default 'private'
    constraint facts_channel_visibility_check check (visibility in ('public', 'private')),
  -- Highest facts_version ever accepted for this channel; maintained by
  -- public.facts_release_guard() and never decreased.
  max_facts_version bigint not null default 0
    constraint facts_channel_max_version_check check (max_facts_version >= 0),
  created_at timestamptz not null default now(),
  constraint facts_channel_scope_slug_key unique (scope, slug),
  -- Only global-scope channels may be world-readable; organization-specific
  -- trust is not implemented (docs/CLOUD_FACTS.md).
  constraint facts_channel_public_is_global_check check (visibility = 'private' or scope = 'global')
);

create table if not exists public.facts_key (
  key_id text primary key
    constraint facts_key_key_id_check check (key_id ~ '^cwf-[a-z0-9-]{1,32}$'),
  scope text not null default 'global'
    constraint facts_key_scope_check check (scope ~ '^[a-z0-9][a-z0-9-]{0,31}$'),
  algorithm text not null default 'ed25519'
    constraint facts_key_algorithm_check check (algorithm = 'ed25519'),
  -- Standard base64 of a raw 32-byte Ed25519 public key. Informational only.
  public_key text not null
    constraint facts_key_public_key_check check (public_key ~ '^[A-Za-z0-9+/]{43}=$'),
  status text not null default 'active'
    constraint facts_key_status_check check (status in ('active', 'retired')),
  created_at timestamptz not null default now()
);

create table if not exists public.facts_release (
  id uuid primary key default gen_random_uuid(),
  channel_id uuid not null
    references public.facts_channel (id) on update cascade on delete restrict,
  -- Positive and inside the JavaScript safe-integer range the clients require.
  facts_version bigint not null
    constraint facts_release_facts_version_check check (facts_version between 1 and 9007199254740991),
  schema_version integer not null default 1
    constraint facts_release_schema_version_check check (schema_version between 1 and 1000),
  envelope_version integer not null default 1
    constraint facts_release_envelope_version_check check (envelope_version between 1 and 1000),
  applies_to text not null
    constraint facts_release_applies_to_check check (
      length(applies_to) between 1 and 200
      and applies_to ~ '^(\*|(>=|<=|>|<|=|\^|~)?\s*\d+(\.\d+){0,2}(-[0-9A-Za-z.-]+)?(\s*,\s*(>=|<=|>|<|=|\^|~)?\s*\d+(\.\d+){0,2}(-[0-9A-Za-z.-]+)?)*)$'
    ),
  key_id text not null
    references public.facts_key (key_id) on update cascade on delete restrict,
  -- Exactly the signed bytes the clients verify: canonical base64, no newline
  -- wrapping, decoding to at most MAX_PAYLOAD_BYTES (512 KiB).
  payload_b64 text not null
    constraint facts_release_payload_b64_check check (
      octet_length(payload_b64) between 4 and 699052
      and octet_length(payload_b64) % 4 = 0
      and payload_b64 ~ '^[A-Za-z0-9+/]+={0,2}$'
      and octet_length(decode(payload_b64, 'base64')) <= 524288
    ),
  -- Base64 of a 64-byte Ed25519 signature.
  sig_b64 text not null
    constraint facts_release_sig_b64_check check (sig_b64 ~ '^[A-Za-z0-9+/]{86}==$'),
  -- Extra rotation signatures; element shape is enforced in the guard trigger.
  sigs jsonb not null default '[]'::jsonb
    constraint facts_release_sigs_check check (jsonb_typeof(sigs) = 'array' and jsonb_array_length(sigs) <= 7),
  -- Operator-visible decode of payload_b64. Never exposed to anon: the signed
  -- bytes are the only representation a client is allowed to consume.
  payload jsonb not null
    constraint facts_release_payload_object_check check (jsonb_typeof(payload) = 'object'),
  -- Derived from the decoded signed payload, so it cannot disagree with it.
  payload_sha256 text generated always as (encode(sha256(decode(payload_b64, 'base64')), 'hex')) stored,
  published_at timestamptz not null,
  not_after timestamptz
    constraint facts_release_not_after_check check (not_after is null or not_after > published_at),
  status text not null default 'published'
    constraint facts_release_status_check check (status in ('published', 'revoked')),
  revoked_at timestamptz,
  revoke_reason text,
  published_by text not null default ''
    constraint facts_release_published_by_check check (length(published_by) <= 200),
  notes text not null default ''
    constraint facts_release_notes_check check (length(notes) <= 4000),
  created_at timestamptz not null default now(),
  constraint facts_release_channel_version_key unique (channel_id, facts_version),
  constraint facts_release_revocation_check check (
    case status
      when 'published' then revoked_at is null and revoke_reason is null
      else revoked_at is not null and length(coalesce(revoke_reason, '')) between 1 and 500
    end
  ),
  -- payload must be the JSON carried by payload_b64, byte for byte after
  -- decoding, so an operator cannot store a second, divergent representation.
  constraint facts_release_payload_bytes_check check (
    convert_from(decode(payload_b64, 'base64'), 'utf8')::jsonb = payload
  ),
  -- Outer columns must repeat what the signed payload says; the clients reject
  -- any envelope whose metadata disagrees with its payload.
  constraint facts_release_payload_meta_check check ((
    payload -> 'facts_version' = to_jsonb(facts_version)
    and payload -> 'schema_version' = to_jsonb(schema_version)
    and payload -> 'applies_to' = to_jsonb(applies_to)
    and payload ->> 'published_at' ~ '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,3})?Z$'
    and (payload ->> 'published_at')::timestamptz = published_at
    and case
          when not_after is null then payload ->> 'not_after' is null
          else payload ->> 'not_after' ~ '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,3})?Z$'
               and (payload ->> 'not_after')::timestamptz = not_after
        end
  ) is true)
);

comment on table public.facts_channel is
  'Cloud facts (facts/v1) delivery channels. Only visibility = ''public'' global-scope channels reach anon through public.facts_current.';
comment on column public.facts_channel.max_facts_version is
  'Never-decreasing high-water mark of accepted facts_version values; blocks re-publication of an older version after revocation.';
comment on table public.facts_key is
  'Operator registry of Ed25519 verifying keys. Informational only: trust comes from the keys pinned in the clients, never from this table, so it is not exposed to anon or authenticated.';
comment on table public.facts_release is
  'Signed facts/v1 envelopes. Append-only apart from revocation; the signed columns are immutable once inserted.';
comment on column public.facts_release.payload is
  'Decoded signed payload for operators. Not granted to anon: clients must consume payload_b64 and verify it.';

-- ---------------------------------------------------------------------------
-- Guard trigger: monotonic versions, channel agreement, signed-row immutability
-- ---------------------------------------------------------------------------

create or replace function public.facts_release_guard()
returns trigger
language plpgsql
security invoker
set search_path = ''
as $facts_release_guard$
declare
  channel_row public.facts_channel%rowtype;
  extra_sig jsonb;
begin
  if tg_op = 'UPDATE' then
    if new.id is distinct from old.id
       or new.channel_id is distinct from old.channel_id
       or new.facts_version is distinct from old.facts_version
       or new.schema_version is distinct from old.schema_version
       or new.envelope_version is distinct from old.envelope_version
       or new.applies_to is distinct from old.applies_to
       or new.key_id is distinct from old.key_id
       or new.payload_b64 is distinct from old.payload_b64
       or new.sig_b64 is distinct from old.sig_b64
       or new.sigs is distinct from old.sigs
       or new.payload is distinct from old.payload
       or new.published_at is distinct from old.published_at
       or new.not_after is distinct from old.not_after then
      raise exception 'signed columns of facts_release % are immutable; publish a higher facts_version instead', old.id
        using errcode = 'restrict_violation';
    end if;
    if old.status = 'revoked' and new.status is distinct from 'revoked' then
      raise exception 'facts_release % cannot be un-revoked; publish a higher facts_version instead', old.id
        using errcode = 'restrict_violation';
    end if;
    return new;
  end if;

  -- Serialize concurrent publications to the same channel.
  select * into channel_row from public.facts_channel where id = new.channel_id for update;
  if not found then
    raise exception 'facts_channel % does not exist', new.channel_id
      using errcode = 'foreign_key_violation';
  end if;

  if new.payload -> 'channel' is distinct from to_jsonb(channel_row.slug) then
    raise exception 'signed payload channel % does not match channel %',
      coalesce(new.payload ->> 'channel', '<missing>'), channel_row.slug
      using errcode = 'check_violation';
  end if;

  if new.facts_version <= channel_row.max_facts_version then
    raise exception 'facts_version % is not above the published high-water mark % for channel %',
      new.facts_version, channel_row.max_facts_version, channel_row.slug
      using errcode = 'unique_violation';
  end if;

  for extra_sig in select value from jsonb_array_elements(new.sigs) loop
    if jsonb_typeof(extra_sig) <> 'object'
       or coalesce(extra_sig ->> 'key_id', '') !~ '^cwf-[a-z0-9-]{1,32}$'
       or coalesce(extra_sig ->> 'sig_b64', '') !~ '^[A-Za-z0-9+/]{86}==$' then
      raise exception 'extra signature % is not a well-formed {key_id, sig_b64} pair', extra_sig
        using errcode = 'check_violation';
    end if;
  end loop;

  update public.facts_channel set max_facts_version = new.facts_version where id = new.channel_id;
  return new;
end;
$facts_release_guard$;

comment on function public.facts_release_guard() is
  'Keeps facts_release versions monotonic per channel, binds the signed payload channel to the channel row, validates extra signature shape, and freezes signed columns after insert.';

drop trigger if exists facts_release_guard on public.facts_release;
create trigger facts_release_guard
  before insert or update on public.facts_release
  for each row execute function public.facts_release_guard();

-- ---------------------------------------------------------------------------
-- Read view: one row per public channel, head version only
-- ---------------------------------------------------------------------------

drop view if exists public.facts_current;
create view public.facts_current
with (security_invoker = true, security_barrier = true) as
select
  c.slug as channel,
  c.scope as scope,
  r.id as release_id,
  r.facts_version,
  r.schema_version,
  r.envelope_version,
  r.applies_to,
  r.key_id,
  r.payload_b64,
  r.sig_b64,
  r.sigs,
  r.payload_sha256,
  r.published_at,
  r.not_after
from public.facts_channel c
join public.facts_release r
  on r.channel_id = c.id
 and r.facts_version = c.max_facts_version
where c.visibility = 'public'
  and r.status = 'published'
  and r.published_at <= now()
  and (r.not_after is null or r.not_after > now());

comment on view public.facts_current is
  'Public read surface for facts/v1: at most one row per public global channel, always the head facts_version. A revoked, future-dated or expired head yields no row rather than an older version.';

-- ---------------------------------------------------------------------------
-- Row level security
-- ---------------------------------------------------------------------------

alter table public.facts_channel enable row level security;
alter table public.facts_key enable row level security;
alter table public.facts_release enable row level security;

drop policy if exists facts_channel_public_read on public.facts_channel;
create policy facts_channel_public_read on public.facts_channel
  for select to anon, authenticated
  using (visibility = 'public');

drop policy if exists facts_release_public_read on public.facts_release;
create policy facts_release_public_read on public.facts_release
  for select to anon, authenticated
  using (
    status = 'published'
    and published_at <= now()
    and (not_after is null or not_after > now())
    and exists (
      select 1 from public.facts_channel c
      where c.id = facts_release.channel_id and c.visibility = 'public'
    )
  );

-- public.facts_key deliberately carries no policy: RLS with no policy denies
-- every anon/authenticated row, and the registry is not a trust root.

-- ---------------------------------------------------------------------------
-- Explicit grants (no wildcards, no writes for anon or authenticated)
-- ---------------------------------------------------------------------------

grant usage on schema public to anon, authenticated, service_role;

revoke all on public.facts_channel from public, anon, authenticated;
revoke all on public.facts_key from public, anon, authenticated;
revoke all on public.facts_release from public, anon, authenticated;
revoke all on public.facts_current from public, anon, authenticated;
revoke all on function public.facts_release_guard() from public;

-- security_invoker views check base-table privileges as the caller, so anon
-- needs column privileges for exactly the columns facts_current reads.
grant select (id, scope, slug, visibility, max_facts_version)
  on public.facts_channel to anon, authenticated;
grant select (
  id, channel_id, facts_version, schema_version, envelope_version, applies_to,
  key_id, payload_b64, sig_b64, sigs, payload_sha256, published_at, not_after, status
) on public.facts_release to anon, authenticated;
grant select on public.facts_current to anon, authenticated;

-- The publisher (web/scripts/facts-publish.mjs) uses the service-role key.
-- No delete on facts_release: publication history is retained, and a bad
-- release is revoked, never erased.
grant select, insert, update, delete on public.facts_channel to service_role;
grant select, insert, update on public.facts_key to service_role;
grant select, insert, update on public.facts_release to service_role;
grant select on public.facts_current to service_role;

-- Channels are created by an authorized operator, one row at a time, and stay
-- private until publication is separately approved:
--   insert into public.facts_channel (scope, slug) values ('global', 'stable');
--   update public.facts_channel set visibility = 'public'
--    where scope = 'global' and slug = 'stable';

notify pgrst, 'reload schema';
