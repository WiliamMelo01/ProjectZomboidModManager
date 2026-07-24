---
name: remote_installation_tolerance
description: Detalhes de arquitetura sobre como o PZMM lida com instalações remotas preexistentes do Project Zomboid pertencentes a outros usuários e locais personalizados no Linux.
---

# Tolerância a Instalações Remotas Preexistentes (PZMM)

Este documento detalha as estratégias implementadas no PZMM (Project Zomboid Mod Manager) para suportar instalações do jogo em servidores remotos Linux que já existem, que foram instaladas fora do padrão do app, ou que pertencem a usuários do sistema diferentes do usuário SSH.

## 1. Múltiplos Diretórios Raiz para Mods (`PZMM_ROOTS`)
Para garantir compatibilidade com diferentes formas de instalação de mods, o sistema varre múltiplos diretórios ao listar e sincronizar os mods:
- Diretório de Workshop do SteamCMD (`remote_steamcmd_home_workshop_dir`)
- Diretório Padrão de Workshop do Steam (`remote_default_steam_workshop_dir`)
- Diretório local de mods do servidor Zomboid (`remote_zomboid_dir/mods`)

O script remoto em Python foi adaptado para ler a variável de ambiente `PZMM_ROOTS` com caminhos separados por quebra de linha em vez de um único `PZMM_ROOT`.

## 2. Estrutura Padrão do Steam Workshop
Durante o upload de mods locais para o servidor remoto, se o mod tiver um `workshop_id` numérico, ele é enviado para a pasta estruturada do Steam (ex: `.../steamapps/workshop/content/108600/<workshop_id>/mods/<nome_mod>`). Isso previne quebras de estrutura em servidores que já utilizam a instalação padrão da Steam e facilita o reconhecimento nativo pelo jogo.

## 3. Gestão de Permissões e `sudo` (`data_owner`)
Para modificar arquivos que pertencem a outro usuário Linux no servidor remoto (ex: um servidor criado pelo usuário `pzserver` acessado via SSH pelo usuário `ubuntu`):
- O Workspace armazena `remote_zomboid_server_owner` e `remote_zomboid_data_owner`.
- As operações que escrevem ou criam pastas utilizam `sudo -n -u "$data_owner"` para criar pastas com o dono correto.
- Utiliza-se `chown -R "$data_owner:$data_owner"` após a cópia de diretórios em áreas de _staging_ antes de aplicar os arquivos no destino.
- **Importante:** O valor do usuário em `linux_sudo_user_arg` *não* deve ser encapsulado por `linux_shell_quote`. Como o OpenSSH no Windows faz um escape automático de aspas simples ao reconstruir os comandos enviados para o servidor, usar `linux_shell_quote` faria o shell remoto interpretar o usuário com aspas literais (ex: `$'pzalt'`), resultando em erros como `chown: invalid user: '$'\''pzalt'\'':$'\''pzalt'\''`. Como o nome de usuário do Linux é validado para conter apenas caracteres alfanuméricos, `_` e `-`, omitir as aspas é seguro e previne esse bug.

## 4. Cache Dinâmico Isolado (`.pzmm-cache`)
Se a pasta Zomboid remotamente configurada não for o padrão gerenciado do PZMM (`/home/pzmm/Zomboid`), o gerenciador criará o diretório de cache (`install-sources`) dinamicamente como uma subpasta `.pzmm-cache` diretamente dentro do diretório do Zomboid fornecido. Isso garante que não haja necessidade de privilégios globais ou colisão de arquivos se houver múltiplos servidores hospedados em contas/caminhos diferentes.

## 5. Logs Relativos ao Diretório do Servidor
No helper remoto, o diretório de Logs do Zomboid é resolvido dinamicamente verificando variáveis de ambiente (`PZMM_DATA_DIR`) ou baseando-se no caminho pai do próprio servidor (`zomboid_server_dir().parent()`), em vez de presumir o diretório `~/Zomboid/Logs` do usuário rodando o script.
