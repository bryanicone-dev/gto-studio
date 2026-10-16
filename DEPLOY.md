# Colocar o GTO Studio online (uso pessoal)

Resultado: o GTO Studio roda 24 horas num servidor gratuito da Oracle, com 4 núcleos e 24 GB de RAM. Seu familiar abre um endereço (por exemplo `https://gto.seudominio.com.br`) no celular ou no PC, entra com um código enviado ao e-mail dele, e o cálculo acontece no servidor. Ninguém mais acessa.

```
Celular / PC ──HTTPS──▶ Cloudflare (login por e-mail) ──túnel──▶ VM Oracle (GTO Studio em 127.0.0.1:7878)
```

| Item | Custo |
|---|---|
| VM Oracle Always Free (4 núcleos ARM, 24 GB) | R$ 0 |
| Cloudflare Tunnel + Access (até 50 usuários) | R$ 0 |
| Domínio (necessário para o login do Cloudflare Access) | ~R$ 40–70 por ano |

Não quer comprar domínio? Veja a **alternativa com Tailscale** no fim: custo zero, mas exige um app instalado no celular e no PC dele.

Tempo total: cerca de 1 a 2 horas na primeira vez.

---

## 1. Conta na Oracle Cloud

1. Crie a conta em <https://www.oracle.com/cloud/free/>. Ela pede cartão só para verificar identidade; o plano Always Free não cobra.
2. **Home region**: escolha **Brazil East (São Paulo)** ou **Brazil Southeast (Vinhedo)**. Os recursos Always Free só existem na home region, e ela não pode ser trocada depois.
3. Recomendado: em *Billing → Upgrade*, passe a conta para **Pay As You Go**. Os recursos Always Free continuam gratuitos, e pelas regras atuais a Oracle deixa de recuperar VMs gratuitas que ficam ociosas. Crie também um **alerta de orçamento** (*Billing → Budgets*) de R$ 1 para ser avisado se algo começar a cobrar. Confira as regras vigentes na página do Always Free.

## 2. Criar a VM

Em *Compute → Instances → Create instance*:

- **Image**: Canonical **Ubuntu 24.04** (a versão aarch64 aparece ao escolher o shape abaixo).
- **Shape**: *Ampere* → **VM.Standard.A1.Flex**, com **4 OCPUs** e **24 GB** de memória (o máximo gratuito).
- **Networking**: deixe criar a VCN padrão com **IP público**.
- **SSH keys**: escolha *Generate a key pair* e **baixe a chave privada** (`.key`). Guarde-a, porque sem ela não há como entrar na VM.
- Disco: o padrão (~50 GB) basta. O gratuito vai até 200 GB.

Se aparecer **"Out of capacity"**, tente outro *Availability Domain* ou tente de novo mais tarde (às vezes leva alguns dias). Nesse meio-tempo, a alternativa é uma VPS paga com Ubuntu: os passos 3 a 7 são idênticos.

Anote o **IP público** da VM.

## 3. Conectar na VM (do seu Windows)

No PowerShell, troque o caminho da chave e o IP:

```bash
ssh -i C:\Users\Icone\Downloads\ssh-key.key ubuntu@IP_DA_VM
```

Se o Windows reclamar das permissões da chave, mova o arquivo para `C:\Users\Icone\.ssh\` e tente de novo.

## 4. Enviar o código

No PowerShell, **na pasta `gto`** da Área de Trabalho. O `--exclude=target` deixa de fora os arquivos de compilação, que ocupam gigabytes:

```bash
tar -czf gto-studio.tgz --exclude=target gto-studio
```

```bash
scp -i C:\Users\Icone\Downloads\ssh-key.key gto-studio.tgz ubuntu@IP_DA_VM:~/
```

## 5. Instalar (dentro da VM)

```bash
tar -xzf gto-studio.tgz && bash gto-studio/deploy/setup.sh
```

O script instala o Rust, compila (a primeira vez leva uns 5 a 10 minutos) e cria o serviço `gto-studio`. Ele sobe sozinho quando a VM liga e reinicia se cair. No fim deve aparecer **"GTO Studio rodando"**.

O servidor escuta só em `127.0.0.1`, ou seja, ainda não é acessível de fora. Isso é proposital: o acesso entra pelo túnel do passo 6. **Não abra a porta 7878** no firewall da Oracle, porque o GTO Studio não tem login próprio.

Comandos úteis na VM:

```bash
sudo systemctl status gto-studio
```

```bash
journalctl -u gto-studio -n 50
```

## 6. Cloudflare: domínio e túnel

1. Crie uma conta grátis em <https://dash.cloudflare.com>.
2. **Domínio**:
   - Compre um `.com` pelo próprio Cloudflare (*Domain Registration*), a preço de custo, ou
   - compre um `.com.br` no <https://registro.br> e, em *Add a site* no Cloudflare, siga as instruções para trocar os *nameservers* no Registro.br.
   - Espere o domínio aparecer como **Active**.
3. Abra **Zero Trust** (menu lateral) e escolha o plano **Free**.
4. Em *Networks → Tunnels → Create a tunnel*:
   - tipo **Cloudflared**, com um nome como `gto-studio`;
   - em ambiente, escolha **Debian** e arquitetura **arm64**;
   - copie o comando de instalação mostrado (ele contém um token) e **rode dentro da VM**. Isso instala o `cloudflared` como serviço.
5. Ainda no túnel, em **Public Hostname**:
   - Subdomain: `gto`; Domain: o seu domínio;
   - Service: **HTTP** → `localhost:7878`.

A partir daqui `https://gto.seudominio` já abre o GTO Studio. **Faça o passo 7 antes de passar o link para alguém.**

## 7. Cloudflare Access: só ele entra

Em *Zero Trust → Access → Applications → Add an application → Self-hosted*:

- **Application domain**: `gto.seudominio`
- **Session duration**: 1 mês, para ele não precisar logar toda hora.
- **Policy**: Action **Allow**, Include **Emails** → o e-mail dele (e o seu, para dar manutenção).
- **Login method**: *One-time PIN* (código por e-mail; já vem ativo).

Teste numa aba anônima: deve aparecer a tela do Cloudflare pedindo e-mail. Um e-mail fora da lista não recebe código.

## 8. Entregar para ele

- Mande o link e explique: "digite seu e-mail e o código que chegar".
- No celular, abra o link e use **"Adicionar à tela inicial"**: vira um atalho como se fosse um app.
- Com 24 GB, o servidor resolve flops com 2 bet sizes e raises (veja a tabela de memória no README). Spots de torneio, com stacks de 15 a 40 bb, são bem mais leves.

## 9. Atualizar a versão

Depois de mudar o código no seu PC, repita o **passo 4** e depois rode na VM:

```bash
tar -xzf gto-studio.tgz && bash gto-studio/deploy/setup.sh
```

A solução aberta no momento se perde ao reiniciar o serviço; é só resolver de novo.

---

## Alternativa sem domínio: Tailscale (custo zero)

O Tailscale cria uma rede privada entre os aparelhos dele e a VM. O plano pessoal é gratuito.

1. Crie uma conta em <https://tailscale.com> com o e-mail dele.
2. Na VM:

   ```bash
   curl -fsSL https://tailscale.com/install.sh | sh
   ```

   ```bash
   sudo tailscale up
   ```

   Abra o link que aparecer e faça login na conta dele.
3. Ainda na VM, publique o GTO Studio só para a rede privada, com HTTPS:

   ```bash
   sudo tailscale serve --bg 7878
   ```

4. Instale o app **Tailscale** no celular e no PC dele, com a mesma conta.
5. Ele acessa pelo endereço mostrado por `tailscale serve status` (algo como `https://nome-da-vm.tailnet.ts.net`).

Prós: sem domínio, nada exposto na internet. Contras: o app do Tailscale precisa estar ligado no aparelho para acessar.

## Problemas comuns

| Sintoma | Causa provável |
|---|---|
| "Out of capacity" ao criar a VM | Falta de máquinas ARM gratuitas na região. Tente outro Availability Domain ou mais tarde. |
| Site mostra erro 502 | O GTO Studio parou. Veja `sudo systemctl status gto-studio` e `journalctl -u gto-studio -n 50`. |
| "A árvore precisa de X GB" | Árvore maior que a RAM livre. Use menos bet sizes ou a precisão de 16 bits. |
| A VM sumiu ou foi desligada | VM gratuita ociosa recuperada pela Oracle. Passe a conta para Pay As You Go (passo 1.3) e recrie. |
| Nenhum código chega no e-mail | O e-mail não está na policy do Access (passo 7) ou foi para o spam. |
