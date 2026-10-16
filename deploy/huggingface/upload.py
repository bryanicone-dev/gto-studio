"""Publica dist/ no Hugging Face Space (usado pelo GitHub Actions).

Variáveis: HF_TOKEN (token Write) e HF_SPACE (ex.: usuario/gto-studio).
Arquivos do site que não existem mais em dist/ são apagados do Space.
"""
import os
import sys

from huggingface_hub import HfApi

token = os.environ.get("HF_TOKEN")
if not token:
    sys.exit("Falta o secret HF_TOKEN no repositório (Settings > Secrets and variables > Actions).")

HfApi(token=token).upload_folder(
    repo_id=os.environ["HF_SPACE"],
    repo_type="space",
    folder_path="dist",
    delete_patterns=["*.html", "*.js", "pkg/**", "library/**"],
    commit_message=f"Publicar GTO Studio ({os.environ.get('GITHUB_SHA', 'local')[:7]})",
)
print(f"Publicado em https://huggingface.co/spaces/{os.environ['HF_SPACE']}")
