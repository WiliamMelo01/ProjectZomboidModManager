---
name: update_download_txt
description: Skill para atualizar os arquivos download.txt na pasta Workshop do PzManager após o lançamento de uma nova release.
---

# Atualização dos arquivos download.txt para nova versão

Sempre que uma nova release do PZ Manager for lançada (ou a versão for atualizada), é necessário atualizar os arquivos `download.txt` localizados dentro da pasta de Workshop do jogo na máquina do usuário para apontar para a nova versão.

## Passos para Atualização

1. A pasta de destino a ser inspecionada é `C:\Users\PC\Zomboid\Workshop\PzManager`.
2. Busque por todos os arquivos chamados `download.txt` de forma recursiva dentro dessa pasta e de suas subpastas.
3. Para cada arquivo encontrado, atualize o conteúdo modificando o número da versão para a nova versão recém-lançada.
4. Caso esteja operando na máquina local (Windows), você pode utilizar ferramentas de leitura/escrita, ou comandos como PowerShell para realizar a substituição do texto de forma automatizada.

**Exemplo de script (PowerShell) para ser utilizado como referência:**
```powershell
$oldVersion = "v0.6.0" # Substitua pela versão antiga
$newVersion = "v0.6.1" # Substitua pela versão nova

Get-ChildItem -Path "C:\Users\PC\Zomboid\Workshop\PzManager" -Filter "download.txt" -Recurse | ForEach-Object {
    $content = Get-Content $_.FullName
    $updatedContent = $content -replace [regex]::Escape($oldVersion), $newVersion
    Set-Content -Path $_.FullName -Value $updatedContent
}
```
> **Nota**: Sempre confirme a versão anterior e a versão atual antes de realizar a substituição no conteúdo dos arquivos.
