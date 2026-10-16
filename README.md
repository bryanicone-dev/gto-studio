# GTO Studio

Solver pós-flop local com visualizador de spots. Você define ranges, board, pote, stack e tamanhos de aposta. O solver calcula o equilíbrio (Discounted CFR, via [postflop-solver](https://github.com/b-inary/postflop-solver)), e o navegador mostra a estratégia em uma grade 13×13. Dá para percorrer a árvore ação por ação e carta por carta.

O mesmo motor tem duas formas de rodar:

| | Versão no navegador (recomendada para uso online) | Servidor nativo |
|---|---|---|
| Onde calcula | No navegador de quem abre (WebAssembly, 1 thread) | Na máquina onde o executável roda (todos os núcleos) |
| Como gerar | `deploy\web\build.ps1` → pasta `dist\` (site estático, ~1,7 MB) | `cargo build --release` → `target\release\gto-studio.exe` |
| Onde hospedar | Grátis: Hugging Face Space Static, Vercel, Cloudflare Pages ([DEPLOY-HUGGINGFACE.md](DEPLOY-HUGGINGFACE.md)) | No próprio PC, ou numa VM/VPS ([DEPLOY.md](DEPLOY.md)) |
| Limite de árvore | ~3 GB no PC, ~1,2 GB no celular | RAM livre da máquina |
| Velocidade (mesmo PC de 4 threads) | turn em ~6 s; flop de 25 bb ~5 s por iteração | turn em ~1,5 s |

As telas, os resultados e as funções são idênticos nas duas versões.

## Publicação automática (GitHub Actions)

O site é montado e publicado **na nuvem**, sem depender de nenhum computador local. A cada push na branch `main`, o workflow [`.github/workflows/publicar.yml`](.github/workflows/publicar.yml):

1. roda os testes;
2. resolve os spots da [biblioteca](deploy/library/spots.json), um preset por máquina, em paralelo. Há cache: só recalcula o que ainda não existe; para refazer tudo, aumente `"version"` no `spots.json`;
3. compila o motor para WebAssembly, monta o site e publica no Space do Hugging Face (`HF_SPACE` no workflow).

Ele precisa de um único secret, **`HF_TOKEN`**: um token "Write" do Hugging Face, cadastrado em *Settings → Secrets and variables → Actions*.

Para mudar algo, edite o arquivo no próprio GitHub (ícone de lápis) e faça o commit. Em 5–15 minutos o site se atualiza. Na primeira vez, ou depois de mudar a biblioteca, leva até ~1 h. Para rodar sem mudar nada: *Actions → Publicar site → Run workflow*.

## Requisitos para compilar

| Item | Versão / observação |
|---|---|
| **Rust** | stable ≥ 1.85 (edition 2024). Instale via [rustup](https://rustup.rs). |
| **Git** | Necessário: o `postflop-solver` é baixado do GitHub na primeira compilação. |
| **Internet** | Só na primeira compilação, para baixar as dependências. Depois funciona offline. |
| **Linker C** | **Windows:** use o toolchain MSVC (precisa do *Visual Studio Build Tools* com "Desenvolvimento para desktop com C++") **ou** o toolchain GNU (`rustup default stable-x86_64-pc-windows-gnu`), que já vem com o próprio linker. **Linux:** `gcc` ou `clang` (pacote `build-essential`). **macOS:** `xcode-select --install`. |

Sistemas suportados: Windows 10/11, Linux e macOS, em x86-64 ou ARM64.

## Requisitos para rodar

- Qualquer navegador moderno (Chrome, Edge, Firefox, Safari).
- **RAM**: é o fator decisivo. A árvore inteira fica em memória, e o tamanho depende de ranges, SPR e, principalmente, da **quantidade de tamanhos de aposta e raise**. O app lê a RAM livre no momento do solve. Se a árvore não couber, ele recusa e informa quanto seria necessário.
- **CPU**: usa todos os núcleos. O tempo cai quase linearmente com o número de núcleos.

### Memória medida (flop K♠7♦2♣, pote 5,5 bb, stack 97,5 bb, ranges padrão do app — mesmos tamanhos nas 3 streets)

| Tamanhos (bet / raise) | 32-bit (precisão total) | 16-bit (comprimido) |
|---|---|---|
| `50%` / sem raise | 0,40 GB | 0,21 GB |
| `33%, 75%` / sem raise | 2,35 GB | 1,21 GB |
| `50%` / `3x` (padrão do app) | 3,31 GB | 1,70 GB |
| `33%` / `3x` | 5,09 GB | 2,62 GB |
| `33%, 75%` / `3x` | 18,5 GB | 9,5 GB |
| `33%, 75%, a` / `2.5x` | 27,3 GB | 14,1 GB |

Spots de **turn** e **river** usam uma fração disso: um turn completo com `50%/3x` ocupou ~0,01 GB e resolveu em 1,5 s.

### Hardware recomendado

| Uso | RAM | CPU |
|---|---|---|
| Turn/river, flops com 1 tamanho | 8 GB | 4 threads |
| Flops com 1–2 tamanhos + raise | **16 GB** | 8+ threads |
| Árvores completas (2–3 tamanhos + raises), em 32-bit | **32–64 GB** | 12+ threads |

Referência medida em notebook com 7,9 GB e 4 threads: flop `50%/3x` (pote 11, stack 92) levou cerca de 2,5 s por iteração. Um solve típico precisa de 200 a 500 iterações para chegar a 0,5% do pote. Com 16 threads e RAM sobrando, o mesmo solve cai para poucos minutos.

## Como compilar e rodar

```bash
cargo build --release
```

```bash
./target/release/gto-studio
```

No Windows o executável é `target\release\gto-studio.exe`. Depois abra `http://127.0.0.1:7878`.

Testes:

```bash
cargo test --release
```

### Configuração (variáveis de ambiente, todas opcionais)

| Variável | Padrão | Função |
|---|---|---|
| `GTO_HOST` | `127.0.0.1` | Endereço em que o servidor escuta. Use `0.0.0.0` só dentro de container (o Dockerfile já faz isso). |
| `GTO_PORT` | `7878` | Porta HTTP. |
| `GTO_MAX_MEMORY_GB` | 85% da RAM livre no momento do solve | Teto de memória por árvore. Aumente por sua conta e risco: acima da RAM física, o sistema passa a usar disco e o solve fica muito lento. |
| `RAYON_NUM_THREADS` | todos os núcleos lógicos | Threads do solver. Reduza se quiser usar o PC durante o solve. |

Para extrair mais velocidade da sua CPU específica (o binário passa a rodar só em CPUs iguais ou mais novas):

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Usando

A interface é em português, com os termos de poker em inglês (Fold, Check, Call, Bet, Raise, All-in, OOP/IP, range, board, EV, equity, combos). Tem cinco telas, acessíveis pela barra superior:

- **Início**: atalhos para cada função, a memória e os núcleos da máquina e o estado da última solução. Os cards marcados "Em breve" (Relatórios, Analisar mãos) ainda não estão implementados.
- **Resolver**: configuração do spot.
  1. **Preset pré-flop**. Preenche ranges, pote e stack. São ranges aproximados de referência, não saída de solver pré-flop.
     - **Cash 100bb (6-max)**: BTN, CO, UTG e SB open vs BB; BB 3-bet vs BTN; BTN 3-bet vs CO.
     - **Torneio (ante de 1bb no BB, já somado ao pote)**: 40bb (BTN e CO open vs BB, BB 3-bet vs BTN), 25bb (BTN e SB open vs BB) e 15bb (BTN open vs BB). O cálculo é em fichas (chip EV), sem ICM.
  2. **Range OOP e range IP**, na notação padrão: `QQ+`, `A5s-A2s`, `KQo`, `AK:0.5` (50% de peso). Intervalos vão do maior para o menor (`TT-22`).
  3. **Board** com 3, 4 ou 5 cartas (`Ks7d2c`). Com 4 ou 5 o cálculo começa no turn ou no river.
  4. **Bet sizes por street**, separados por vírgula: bet em `%` do pote, `e` (geométrico) ou `a` (all-in); raise em `3x` (o bet anterior), `%` do pote ou `a`. Campo vazio = sem raise.
  5. **Opções avançadas**: rake e rake cap, donk bets, add/force all-in, merging threshold e precisão numérica. A precisão **automática** usa 32 bits quando cabe e 16 bits quando não cabe.
  6. **Resolver** mostra a iteração e a exploitability em % do pote. O cálculo para sozinho ao atingir a exploitability alvo e abre a tela Estudar. **Parar e ver parcial** interrompe e libera a estratégia atual.
- **Biblioteca** (versão no navegador): spots de torneio já resolvidos que abrem na hora.
  - Cada spot guarda só as decisões do flop (~64 KB). Ao escolher a carta do turn, o app resolve aquela street no navegador (~7 s num PC comum), e o river depois dela já vem junto.
  - Inclui 6 flops (A-high seco, K-high, broadways two-tone, conectado, pareado e monotone baixo) para **25bb** e **15bb** BTN open vs BB call, com bet 50% e raise 3x.
  - Para gerar mais spots, edite `deploy/library/spots.json` e rode `cargo run --release --bin library -- deploy/library/spots.json library`. Spots já prontos são pulados. Depois publique de novo.
- **Estudar**: a solução.
  - **Barra da mesa** no topo (quando o spot vem de um preset): os 6 lugares, a ação pré-flop de cada um, o stack no início do flop e quem age agora.
  - Os botões de ação (Fold, Check, Call, Bet, Raise, All-in) avançam na árvore e mostram frequência e EV de cada ação.
  - Nos pontos de turn e river você escolhe a carta.
  - O caminho no topo volta a qualquer decisão. A URL guarda o caminho (`#/estudar/0.1.48`): **Copiar link do spot** reabre o mesmo ponto, e o botão voltar do navegador funciona. O link vale enquanto a mesma solução estiver aberta.
  - **Aba Estratégia**: na grade 13×13, as cores mostram o mix de ações e a altura mostra quanto da mão ainda está no range. Clicar numa mão abre frequência e EV por ação de cada combo, com a categoria dele.
  - **Filtros por categoria**: made hands (Set, Two pair, Overpair, Top pair, Middle pair, Weak pair, Ace high…) e draws (Flush draw, OESD, Gutshot, Backdoor flush draw, Overcards), cada um com combos e mix de ações. Clicar destaca as mãos na grade.
  - **Aba Ranges e equity**: os ranges dos dois jogadores lado a lado, coloridos por equity, e o gráfico de distribuição de equity.
- **Treinar**: mãos sorteadas da solução aberta.
  - Você escolhe jogar como OOP, IP ou aleatório. Sua mão e a do vilão são sorteadas pelos ranges, e as cartas de turn e river saem aleatórias.
  - O vilão joga a estratégia GTO do combo dele.
  - Cada decisão sua recebe nota pelo mix e pelo EV do seu combo: **Melhor jogada** (a ação mais frequente), **Correta** (usada em 5% ou mais), e abaixo disso **Imprecisa** (perde menos de 2% do pote), **Erro** (menos de 10%) ou **Erro grave**.
  - A sessão soma mãos, notas e EV perdido. Cada feedback tem um link para o spot no Estudar.
  - Se você escolher uma ação que o GTO nunca usa, a mão sai da linha de equilíbrio e é encerrada.

## Limitações conhecidas

- **Só pós-flop e heads-up** (2 jogadores). Os ranges de entrada vêm de você ou de uma tabela pré-flop. O postflop-solver não resolve pré-flop nem multiway.
- **Um solve por vez.** Rodar um novo solve (ou abrir um spot da biblioteca) descarta o anterior. Para salvar e carregar árvores, o projeto fixa o `bincode` na versão 2.0.0-rc.3, que é a que o postflop-solver usa; a 2.x estável é incompatível.
- **Turn da biblioteca é resolvido como subjogo**: a estratégia de turn e river vem de um novo cálculo a partir dos ranges que chegam ao turn, com os mesmos bet sizes. É o método padrão de "re-solve", mas pode diferir um pouco da árvore completa.
- O `postflop-solver` está com o desenvolvimento suspenso desde 2023. Funciona e é rápido, mas não recebe correções.
- **Licença AGPL-3.0** (herdada do postflop-solver). Uso pessoal é livre. Se oferecer o app como serviço web, o código-fonte precisa ser disponibilizado.

## Estrutura

```
src/engine.rs      motor: monta e resolve a árvore e gera o JSON de cada nó (com testes)
src/hands.rs       categorias de mão (made hands e draws) em relação ao board (com testes)
src/main.rs        servidor HTTP nativo (usa o motor)
web/               versão no navegador: o motor exposto ao WebAssembly
static/index.html  interface (embutida no executável e copiada para dist\)
static/worker.js   Web Worker que roda o WebAssembly fora da tela
deploy/web         build do site estático (build.ps1) e servidor de teste (serve.mjs)
src/bin/library.rs gerador da biblioteca (resolve e salva só o flop de cada spot)
deploy/library     lista de spots da biblioteca (spots.json)
library/           spots gerados (index.json + .bin), copiados para o site no build
deploy/huggingface cartão do Space Static e script de publicação
deploy/            também: serviço systemd e script de instalação para servidor Linux
Dockerfile         servidor nativo em container (para hosts com Docker)
DEPLOY-HUGGINGFACE.md  colocar online de graça (versão no navegador)
DEPLOY.md          alternativa: servidor Linux (Oracle grátis ou VPS) + Cloudflare
```

API local (JSON): `POST /api/solve`, `GET /api/status`, `POST /api/cancel`, `POST /api/node {history: [índices]}`.
