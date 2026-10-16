# Publica o GTO Studio (versão no navegador) num Hugging Face Space do tipo Static, que é gratuito.
# Uso, na pasta gto-studio:
#   powershell -ExecutionPolicy Bypass -File deploy\huggingface\publish.ps1 -Space seu-usuario/gto-studio
# No primeiro acesso o Git pede login: usuário do Hugging Face e, como senha, um Access Token "Write".
param([Parameter(Mandatory)][string]$Space)
$ErrorActionPreference = 'Stop'

$project = (Resolve-Path "$PSScriptRoot\..\..").Path
$repo = Join-Path (Split-Path $project) 'hf-space'

# 1. Gera o site estático em dist\
& powershell -ExecutionPolicy Bypass -File "$project\deploy\web\build.ps1"
if ($LASTEXITCODE -ne 0) { throw 'O build do site falhou.' }

# 2. Clona o Space (uma vez)
if (-not (Test-Path "$repo\.git")) {
    git clone "https://huggingface.co/spaces/$Space" $repo
    if ($LASTEXITCODE -ne 0) { throw "Não foi possível clonar o Space '$Space'. Confira o nome (usuario/space) e o login." }
}
# Os arquivos já usam LF; sem conversão para CRLF nem avisos.
git -C $repo config core.autocrlf false
# O Hugging Face exige Git LFS para arquivos binários, como o .wasm.
git -C $repo lfs install --local | Out-Null
git -C $repo lfs track '*.wasm' | Out-Null
git -C $repo lfs track '*.bin' | Out-Null

# 3. Espelha dist\ no Space, mantendo o histórico e o .gitattributes
Get-ChildItem $repo -Force | Where-Object { $_.Name -notin '.git', '.gitattributes' } | Remove-Item -Recurse -Force
Copy-Item "$project\dist\*" $repo -Recurse
Copy-Item "$PSScriptRoot\README.md" $repo

if (-not (git -C $repo config user.email)) {
    git -C $repo config user.name 'GTO Studio'
    git -C $repo config user.email 'gto-studio@users.noreply.huggingface.co'
}
git -C $repo add -A
if (-not (git -C $repo status --porcelain)) {
    Write-Host 'Nada mudou desde a última publicação.'
    exit 0
}
git -C $repo commit -m "Publicar GTO Studio $(Get-Date -Format 'yyyy-MM-dd HH:mm')"
git -C $repo push
if ($LASTEXITCODE -ne 0) { throw 'O push falhou. Confira o token (precisa de permissão Write).' }
Write-Host "Publicado. Abra https://huggingface.co/spaces/$Space"
