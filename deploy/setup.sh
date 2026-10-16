#!/usr/bin/env bash
# Instala ou atualiza o GTO Studio num servidor Ubuntu (ARM ou x86-64).
# Uso, dentro da pasta gto-studio copiada para o servidor:  bash deploy/setup.sh
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> Dependências do sistema"
sudo apt-get update -y
sudo apt-get install -y build-essential pkg-config git curl

echo "==> Rust"
if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup update stable

echo "==> Compilando (a primeira vez leva alguns minutos)"
cargo build --release

echo "==> Instalando o serviço"
id gto >/dev/null 2>&1 || sudo useradd --system --no-create-home --shell /usr/sbin/nologin gto
sudo install -D -m 755 target/release/gto-studio /opt/gto-studio/gto-studio
sudo install -m 644 deploy/gto-studio.service /etc/systemd/system/gto-studio.service
sudo systemctl daemon-reload
sudo systemctl enable gto-studio
sudo systemctl restart gto-studio

echo "==> Verificando"
for _ in 1 2 3 4 5; do
  if curl -fsS http://127.0.0.1:7878/api/status >/dev/null; then
    echo "GTO Studio rodando em http://127.0.0.1:7878 (acessível de fora só pelo túnel)."
    exit 0
  fi
  sleep 1
done
echo "O serviço não respondeu. Veja os logs com: journalctl -u gto-studio -n 50" >&2
exit 1
