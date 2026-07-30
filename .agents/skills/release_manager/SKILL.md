---
name: release_manager
description: Orquestra o processo de lançamento de novas versões (releases), automatizando bumps de versão, changelog e geração de BBCode para a Steam, exceto a criação da release no GitHub.
---

# Release Manager Skill

Sempre que o usuário solicitar o lançamento de uma nova versão (ex: "vamos lançar uma nova release"), você deve utilizar esta skill e seguir rigorosamente os passos abaixo em ordem.

## 1. Confirmar a Numeração e Conteúdo
- Pergunte ao usuário qual será a numeração da nova release (ex: `0.6.2`).
- Pergunte também um resumo das principais mudanças que devem entrar nas notas de atualização (patch notes), caso a conversa atual já não deixe isso claro.
- **PARE e aguarde a resposta/confirmação do usuário antes de prosseguir para os próximos passos.**

## 2. Atualizar Versões Locais
Modifique os seguintes arquivos no repositório para refletir a nova versão:
- `package.json` (na raiz do projeto).
- `src-tauri/tauri.conf.json` (na pasta `src-tauri`).

## 3. Atualizar o CHANGELOG.md
- Adicione as notas da nova versão no arquivo `CHANGELOG.md` na raiz do projeto, mantendo a formatação e o histórico padrão do arquivo.

## 4. Atualizar o arquivo download.txt (Steam Workshop)
- Invoque e siga as instruções da skill `update_download_txt` (localizada em `.agents/skills/update_download_txt/SKILL.md`) para atualizar a URL da versão no arquivo `download.txt` dentro da pasta local do mod da Steam.

## 5. Gerar Códigos BB para a Steam
Gere e exiba para o usuário na sua resposta (em blocos de código separados) o BBCode necessário para ele copiar e colar na Steam:
- **Patch Notes (Steam):** Um texto formatado em BBCode detalhando as mudanças da versão (ex: usando `[list]`, `[*]`, `[b]`).
- **Descrição da Steam (Workshop):** A descrição completa do mod na Steam formatada em BBCode. Lembre-se de atualizar qualquer menção estática de versão (se houver) no texto gerado, baseando-se na descrição usada anteriormente.

## 6. Commit e Push (Git)
- Execute `git add package.json src-tauri/tauri.conf.json CHANGELOG.md <caminho_do_download.txt>`
- Execute `git commit -m "chore: release vX.X.X"` (substituindo pelo número da nova versão).
- Execute `git push` para enviar as alterações para o repositório remoto.

> **IMPORTANTE:** O passo de criar e publicar a Release de fato no GitHub (que compila os binários via CI/CD) **NÃO** deve ser feito por você. O usuário fará isso manualmente. Seu trabalho termina no `git push`.
