#!/usr/bin/env bash
# Sincroniza o repositório local com o remoto (origin).
# Equivale a uma cópia exata do GitHub — apaga qualquer mudança local.
set -e

cd "$(dirname "$0")/.."

REMOTE="origin"
BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null || echo "main")

echo "==> Buscando atualizações de $REMOTE/$BRANCH..."
git fetch "$REMOTE"

echo "==> Resetando para $REMOTE/$BRANCH (descarta commits e mudanças locais)..."
git reset --hard "$REMOTE/$BRANCH"

echo "==> Removendo arquivos não rastreados e ignorados..."
git clean -fdx

echo ""
echo "Local agora é cópia exata de $REMOTE/$BRANCH."
