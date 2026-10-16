# Colocar o GTO Studio online no Hugging Face (grátis)

Resultado: um site estático no Hugging Face, num **Space do tipo Static**, gratuito. O cálculo do solver roda **no navegador de quem abre o site** (WebAssembly), então não há servidor, cartão nem custo, e o site não "dorme".

O que esperar da versão no navegador:

| Spot | PC comum | Celular |
|---|---|---|
| River e turn | segundos | segundos a ~1 min |
| Flop de torneio (15–40 bb) | ~5–15 min | pode ficar lento ou faltar memória |
| Flop de cash 100 bb, poucos bet sizes | ~10–30 min | não recomendado |

- A **Biblioteca** abre spots de torneio na hora (o flop já vem resolvido) e calcula o turn em ~7 s quando a carta é escolhida. Ela é publicada junto com o site.
- O limite é de **~3 GB por árvore no PC** e **~1,2 GB no celular** (teto do navegador). Árvores maiores são recusadas com uma mensagem.
- Para flops, acelere usando **exploitability alvo de 1%** em vez de 0,5%, ou **"Parar e ver parcial"** quando a estratégia já estiver estável.
- A aba precisa ficar aberta enquanto calcula.

Tempo total do setup: uns 10 minutos.

---

## 1. Conta

<https://huggingface.co/join>: crie a conta e confirme o e-mail. Anote o **nome de usuário**.

## 2. Criar o Space

<https://huggingface.co/new-space>:

| Campo | Valor |
|---|---|
| Owner | seu usuário |
| Space name | `gto-studio` |
| License | `agpl-3.0` |
| Select the Space SDK | **Static** → template **Blank** |
| Visibility | **Private** (veja o passo 6) |

Clique em **Create Space**.

## 3. Token de acesso

1. <https://huggingface.co/settings/tokens> → **Create new token**.
2. Tipo **Write**, nome `publicar-gto` → **Create token**.
3. Copie o token (`hf_...`) e guarde. **Não mande para ninguém, nem para mim.**

## 4. Publicar

No PowerShell (troque `SEU_USUARIO`):

```bash
cd C:\Users\Icone\Desktop\gto\gto-studio
```

```bash
powershell -ExecutionPolicy Bypass -File deploy\huggingface\publish.ps1 -Space SEU_USUARIO/gto-studio
```

O script:
1. gera o site em `dist\` (compila o motor para WebAssembly);
2. clona o Space em `C:\Users\Icone\Desktop\gto\hf-space`;
3. copia os arquivos e envia.

Quando o Git pedir login (janela do Git Credential Manager ou no terminal):
- **usuário**: seu usuário do Hugging Face;
- **senha**: o token `hf_...`.

No fim aparece **"Publicado"**. Como não há compilação no Hugging Face, o Space fica pronto em segundos.

## 5. Testar

Abra `https://huggingface.co/spaces/SEU_USUARIO/gto-studio`. Na tela inicial, "Limite no navegador" deve mostrar ~3 GB no PC. Faça um teste rápido:
- Resolver → preset **25bb · BTN open 2 vs BB call**;
- board `Ks7d2c8h` (turn) → **Resolver**: termina em segundos e abre o Estudar.

## 6. Acesso do seu familiar

**Opção A, privada (recomendada)**
1. Ele cria uma conta no Hugging Face com o e-mail dele.
2. Você cria uma organização gratuita: <https://huggingface.co/organizations/new> (ex.: `gto-familia`) e o convida em **Settings → Members → Invite**, com papel **read**.
3. No Space: **Settings → Rename or transfer this space** → owner `gto-familia`.
4. Ele abre logado: `https://huggingface.co/spaces/gto-familia/gto-studio`.
5. Para publicar depois disso, apague a pasta `C:\Users\Icone\Desktop\gto\hf-space` e use `-Space gto-familia/gto-studio`.

**Opção B, pública (mais simples)**
Em **Settings → Change Space visibility → Public**, qualquer pessoa com o link usa, sem login. Não existe dado pessoal nenhum no app, porque o cálculo roda no aparelho de quem abre. O endereço em tela cheia aparece no menu **⋮ → Embed this Space** do Space.

**No celular**: abra o link e use **"Adicionar à tela inicial"**.

## 7. Atualizar

Mudou algo no projeto? Rode o comando do passo 4 de novo.

## Outras hospedagens

A pasta `dist\` é um site estático comum. Também funciona em:
- **Vercel**: `npx vercel deploy dist --prod`; o plano Hobby é gratuito para uso pessoal.
- **Cloudflare Pages** ou **Netlify**: arraste a pasta `dist` no painel.

Para testar localmente:

```bash
node deploy/web/serve.mjs dist 8080
```

Depois abra `http://127.0.0.1:8080`.

## Problemas comuns

| Sintoma | O que fazer |
|---|---|
| `Authentication failed` | O token precisa ser **Write**. Se o Windows guardou um login errado, apague em *Gerenciador de Credenciais → Credenciais do Windows → git:https://huggingface.co*. |
| `Repository not found` | Confira `SEU_USUARIO/gto-studio`. |
| Push recusado por "binary files" | O script já envia o `.wasm` via Git LFS. Confira se `git lfs version` funciona no PowerShell. |
| "O navegador interrompeu o cálculo" | Faltou memória no aparelho. Use menos bet sizes, ranges menores ou a precisão de 16 bits. |
| A tela fica em "Resolvendo" e não avança | Em celulares, a aba em segundo plano é pausada. Mantenha o app aberto na tela. |
