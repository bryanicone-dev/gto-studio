# Gera o site estático do GTO Studio (solver no navegador, via WebAssembly) em dist\.
# Uso, na pasta gto-studio:
#   powershell -ExecutionPolicy Bypass -File deploy\web\build.ps1
# Requisitos: Rust com o alvo wasm32-unknown-unknown e o wasm-bindgen CLI na mesma versão do crate
# (veja web\Cargo.lock). Por padrão o script procura o CLI em %USERPROFILE%\.gto-tools.
param([string]$WasmBindgen = "$env:USERPROFILE\.gto-tools\wasm-bindgen-0.2.129-x86_64-pc-windows-msvc\wasm-bindgen.exe")
$ErrorActionPreference = 'Stop'

$project = (Resolve-Path "$PSScriptRoot\..\..").Path
$dist = Join-Path $project 'dist'
$cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargo) { $cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe" }
if (-not (Test-Path $WasmBindgen)) { throw "wasm-bindgen não encontrado em $WasmBindgen" }

Write-Host '==> Compilando o motor para WebAssembly'
& $cargo build --release --target wasm32-unknown-unknown --manifest-path "$project\web\Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw 'A compilação falhou.' }

Write-Host '==> Gerando a ponte JavaScript'
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
& $WasmBindgen --target web --no-typescript --out-dir "$dist\pkg" "$project\web\target\wasm32-unknown-unknown\release\gto_studio_web.wasm"
if ($LASTEXITCODE -ne 0) { throw 'O wasm-bindgen falhou.' }

Write-Host '==> Montando o site'
$html = [IO.File]::ReadAllText("$project\static\index.html")
$marker = "const BACKEND = 'server';"
if (-not $html.Contains($marker)) { throw "Marcador '$marker' não encontrado em static\index.html" }
[IO.File]::WriteAllText("$dist\index.html", $html.Replace($marker, "const BACKEND = 'wasm';"), [Text.UTF8Encoding]::new($false))
foreach ($f in 'worker.js', 'sw.js', 'manifest.json', 'icon.svg') { Copy-Item "$project\static\$f" $dist }
# Tabela de equity do Push/fold (gerada por: node deploy/pushfold/equity.mjs pushfold/equity.bin)
if (Test-Path "$project\pushfold\equity.bin") { Copy-Item "$project\pushfold\equity.bin" "$dist\pushfold-equity.bin" }
# Spots já resolvidos (gerados por: cargo run --release --bin library -- deploy/library/spots.json library)
if (Test-Path "$project\library\index.json") { Copy-Item "$project\library" "$dist\library" -Recurse }

$size = (Get-ChildItem $dist -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
Write-Host ("Pronto: {0} ({1:N1} MB). Publique a pasta inteira em qualquer hospedagem estática." -f $dist, $size)
