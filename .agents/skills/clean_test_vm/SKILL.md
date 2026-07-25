---
name: clean_test_vm
description: Instruções de como limpar uma máquina virtual de testes apagando o servidor, arquivos do Project Zomboid, vestígios do app PZ Manager, caches e steamcmd.
---

# Limpeza de Máquina Virtual de Testes (PZMM)

Para resetar a VM de testes e simular uma instalação "limpa", garantindo que o Project Zomboid Mod Manager não possua nenhum estado pré-existente (caches, configurações ou dependências instaladas), execute os seguintes passos na máquina virtual (Ubuntu/Debian) usando privilégios de `root` ou `sudo`.

## 1. Parar e Remover Serviços Systemd do App

O aplicativo pode ter criado daemons de sistema para gerenciar o servidor Zomboid.

```bash
# Pare todos os serviços criados pelo pzmm
sudo systemctl stop pzmm-*.service 2>/dev/null || true
sudo systemctl disable pzmm-*.service 2>/dev/null || true

# Remova os arquivos de serviço do systemd
sudo rm -f /etc/systemd/system/pzmm-*.service
sudo rm -f /etc/systemd/system/pzmm-*.socket

# Recarregue os daemons do systemd para aplicar as remoções
sudo systemctl daemon-reload
```

## 2. Remover Arquivos e Pastas do Aplicativo (PZMM)

O app cria um usuário `pzmm` e utiliza diretórios em `/var/lib` e `/opt`.

```bash
# Remover o diretório de dados gerenciados, steamcmd e servidor (tudo que fica em /var/lib/pzmm)
sudo rm -rf /var/lib/pzmm

# Remover o helper binário do Linux (Tauri Sidecar)
sudo rm -rf /opt/pzmm

# Remover o usuário de sistema pzmm e seu grupo
sudo userdel -r pzmm 2>/dev/null || true
sudo groupdel pzmm 2>/dev/null || true
```

## 3. Limpar Arquivos Temporários

Algumas transferências de instalação e zips podem ficar presas na pasta `/tmp`.

```bash
sudo rm -f /tmp/pzmm-*.txt
sudo rm -f /tmp/pzmm-*.zip
sudo rm -f /tmp/pzmm-*.tar.gz
```

## 4. Remover Caches Locais e Instalações em Todos os Usuários (/home)

A limpeza deve deixar estritamente apenas o usuário base (`ubuntu`). Certifique-se de limpar os diretórios em TODAS as pastas de usuário que tenham qualquer relação com o Project Zomboid ou o SteamCMD, além de deletar usuários secundários (como `pzalt`) criados para testes.

```bash
# Apagar dados do Zomboid, SteamCMD e pzserver do usuário atual (ubuntu)
sudo rm -rf /home/ubuntu/Zomboid
sudo rm -rf /home/ubuntu/pzserver
sudo rm -rf /home/ubuntu/.pzmm-cache
sudo rm -rf /home/ubuntu/Steam
sudo rm -rf /home/ubuntu/steamcmd
sudo rm -rf /home/ubuntu/.local/share/Steam

# Listar uso de disco em /home para encontrar outros usuários e pastas grandes
sudo du -h --max-depth=2 /home | sort -h

# Apagar usuários secundários de teste (ex: pzalt) e todas as suas pastas
sudo userdel -r pzalt 2>/dev/null || true
```

## 5. Limpar Configurações e Cache no Aplicativo Local (Host Windows/Linux)

Para que o teste de fato comece do zero e o app não lembre que este servidor remoto já existiu, é necessário limpar os dados **dentro do próprio PZ Manager na sua máquina (Host)**.

1. **Apagar do Workspace Selector**: Abra a tela de seleção de Workspace, clique nos "Três Pontinhos" na conexão remota salva e escolha **"Excluir Workspace"**. Isso apaga o arquivo `remote-workspace-<hash>.ini` na pasta AppData/Config.
2. **Limpar Cache Front-end**: Vá na tela de `Configurações` (Settings) dentro do app local e clique no botão **"Limpar Cache e Reiniciar"** (ou aperte `F12` > Application > Local Storage > e exclua todas as chaves que começam com `pzmm:`). Isso elimina o histórico de servidores listados, mods oxidados da biblioteca remota, etc.

Após executar todos esses comandos na VM e realizar a limpeza no aplicativo local, o ambiente estará 100% livre de vestígios antigos do PZ Manager, do Zomboid e do SteamCMD. O próximo teste de instalação se comportará como se a máquina acabasse de ser criada na nuvem.
