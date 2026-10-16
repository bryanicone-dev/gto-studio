-- Sincronização do GTO Studio entre aparelhos (Supabase, plano gratuito).
-- Rode uma vez no SQL Editor do projeto. O site chama só as duas funções abaixo com a chave
-- publishable (pública por natureza); a tabela não fica acessível diretamente.
-- Cada aparelho guarda um código aleatório de 20 caracteres; o banco guarda só o hash dele.

create table if not exists public.sync_data (
  code_hash text primary key,
  data jsonb not null,
  updated_at timestamptz not null default now()
);

-- RLS ligada e sem políticas: ninguém lê ou grava a tabela direto, só pelas funções.
alter table public.sync_data enable row level security;

create or replace function public.sync_get(code text)
returns jsonb
language sql
security definer
set search_path = ''
as $$
  select data from public.sync_data
  where code_hash = encode(sha256(convert_to(code, 'UTF8')), 'hex');
$$;

create or replace function public.sync_put(code text, payload jsonb)
returns void
language plpgsql
security definer
set search_path = ''
as $$
begin
  if length(code) < 20 then
    raise exception 'código curto demais';
  end if;
  if pg_column_size(payload) > 1000000 then
    raise exception 'dados grandes demais';
  end if;
  insert into public.sync_data (code_hash, data, updated_at)
  values (encode(sha256(convert_to(code, 'UTF8')), 'hex'), payload, now())
  on conflict (code_hash) do update set data = excluded.data, updated_at = now();
end;
$$;

revoke all on function public.sync_get(text) from public;
revoke all on function public.sync_put(text, jsonb) from public;
grant execute on function public.sync_get(text) to anon, authenticated;
grant execute on function public.sync_put(text, jsonb) to anon, authenticated;
