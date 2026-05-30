<div align="center">

# 🧟 Zomboid Server Mod Manager

### Gerencie mods de servidores multiplayer de Project Zomboid sem editar configurações na mão.

[![Versão](https://img.shields.io/badge/versão-0.0.1-6d5dfc?style=for-the-badge)](package.json)
![Plataforma](https://img.shields.io/badge/plataforma-Windows-0078D4?style=for-the-badge&logo=windows)
![Desktop](https://img.shields.io/badge/desktop-Tauri-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)
![Status](https://img.shields.io/badge/status-em%20desenvolvimento-F59E0B?style=for-the-badge)

</div>

---

## 😵 O problema

Gerenciar mods em um servidor multiplayer de **Project Zomboid** pode ficar complicado rapidamente. Cada novo mod traz IDs, itens da Workshop, dependências e uma posição correta na ordem de carregamento. Basta uma entrada ausente ou fora de ordem para impedir o servidor de iniciar ou para fazer os jogadores perderem tempo tentando descobrir o que deu errado.

O **Zomboid Server Mod Manager** centraliza esse trabalho em uma interface desktop. Ele encontra seus servidores e mods, atualiza os arquivos `.ini`, ajuda a organizar dependências, baixa itens da Steam Workshop e testa a inicialização do servidor com logs em tempo real.

> Menos tempo editando `Mods=` e `WorkshopItems=` manualmente. Mais tempo jogando.

## ✨ Funcionalidades

| | Recurso | O que você pode fazer |
| :---: | --- | --- |
| 🖥️ | **Servidores** | Criar perfis, listar servidores existentes, pesquisar, ocultar perfis e clonar listas de mods. |
| 🧩 | **Mods ativos** | Ativar, desativar e reorganizar mods com atualização automática do arquivo `.ini`. |
| 🔗 | **Dependências** | Detectar dependências ausentes, instalar itens necessários e manter a ordem correta de carregamento. |
| 📚 | **Biblioteca** | Encontrar mods locais, itens da Steam Workshop e mods armazenados em pastas personalizadas. |
| ⬇️ | **Downloads** | Baixar um mod individual ou uma coleção completa por ID ou URL usando SteamCMD com login anônimo. |
| 🧪 | **Diagnóstico** | Testar a inicialização do servidor, acompanhar logs em tempo real e identificar conflitos de porta. |
| ⚙️ | **Configurações** | Detectar o Project Zomboid e o SteamCMD, ajustar RAM e adicionar diretórios monitorados. |

<details>
<summary><strong>🖥️ Gerenciamento de servidores</strong></summary>
<br>

- Lista os perfis encontrados em `Zomboid/Server`.
- Exibe nome, arquivo `.ini`, porta, limite de jogadores e quantidade de mods ativos.
- Cria servidores a partir dos arquivos de exemplo incluídos no projeto.
- Permite selecionar mods durante a criação ou clonar a lista de outro servidor.
- Oculta perfis da listagem principal sem excluir seus arquivos.
- Pesquisa servidores por nome, arquivo ou porta.

</details>

<details>
<summary><strong>📚 Biblioteca de mods</strong></summary>
<br>

- Procura mods locais, itens da Steam Workshop e pastas personalizadas.
- Exibe nome, autor, versão, descrição, tamanho, Mod ID e Workshop ID.
- Pesquisa por nome, autor, descrição, IDs e dependências.
- Filtra mods locais e itens disponíveis na Steam.
- Copia mods baixados pela Steam para a pasta local do Project Zomboid.
- Importa todos os mods disponíveis na Steam para a biblioteca local de uma vez.
- Lê imagens definidas como `poster` ou `icon` no arquivo `mod.info`.

</details>

<details>
<summary><strong>🔗 Mods ativos e dependências</strong></summary>
<br>

- Atualiza automaticamente os campos `Mods` e `WorkshopItems` do arquivo `.ini`.
- Ativa, desativa e reorganiza mods por servidor.
- Mantém dependências ativas antes dos mods que dependem delas.
- Alerta antes de desativar um mod utilizado por outros mods ativos.
- Detecta dependências disponíveis e oferece instalação e ativação em conjunto.
- Identifica dependências ausentes e ajuda a localizar ou baixar o item correspondente.

</details>

<details>
<summary><strong>⬇️ Steam Workshop e SteamCMD</strong></summary>
<br>

- Aceita Workshop ID numérico ou URL completa.
- Baixa um item individual ou resolve e baixa todos os itens de uma coleção pública.
- Exibe o progresso do download de coleções item a item.
- Usa uma única sessão SteamCMD para baixar coleções com mais rapidez.
- Permite cancelar downloads e repetir somente os itens que falharam.
- Oferece validação completa opcional para investigar downloads corrompidos.
- Atualiza automaticamente a biblioteca de mods após o download.
- Mantém downloads em segundo plano ao navegar entre as telas do aplicativo.
- Exibe progresso compacto no canto da tela e envia uma notificação ao finalizar.
- Executa downloads pelo SteamCMD com login anônimo.
- Inclui um `steamcmd.zip` gerenciado nos recursos do aplicativo.
- Detecta instalações existentes e permite selecionar o executável manualmente.
- Abre itens da Workshop no navegador, no cliente Steam ou em uma janela auxiliar.

</details>

<details>
<summary><strong>🧪 Configurações e diagnóstico</strong></summary>
<br>

- Detecta a instalação do Project Zomboid e permite selecionar o executável manualmente.
- Configura a RAM do cliente e do servidor alterando flags `-Xms` e `-Xmx`.
- Exibe os locais monitorados e permite adicionar diretórios personalizados.
- Testa a inicialização de um servidor com logs em tempo real.
- Valida dependências ausentes e ordem incorreta antes do teste.
- Verifica conflitos de porta e permite encerrar processos conflitantes.
- Exibe notificações quando um teste minimizado termina.
- Mantém o progresso de testes minimizados em um cartão compacto no canto da tela.

</details>

## 🚀 Primeiros passos

1. Abra **Configurações** e confira se o SteamCMD foi localizado automaticamente.
2. Verifique se o executável do Project Zomboid foi detectado.
3. Adicione diretórios personalizados caso mantenha mods fora das pastas padrão.
4. Atualize a biblioteca para listar mods locais e itens encontrados na Steam.
5. Crie um servidor selecionando mods ou clonando a lista de outro perfil.
6. Revise dependências, ajuste a ordem dos mods e execute um teste de inicialização.

## 📸 Interface

As capturas de tela oficiais serão adicionadas ao repositório conforme a interface evoluir.

| Tela | Descrição | Caminho planejado |
| --- | --- | --- |
| Dashboard | Servidores e criação de novos perfis | `docs/screenshots/dashboard.png` |
| Detalhes do servidor | Mods ativos, disponíveis e ações de gerenciamento | `docs/screenshots/server-detail.png` |
| Biblioteca | Busca, filtros e importação de mods | `docs/screenshots/mods.png` |
| Downloads | Download por Workshop ID ou URL | `docs/screenshots/downloads.png` |
| Configurações | SteamCMD, diretórios, executável e RAM | `docs/screenshots/settings.png` |
| Teste do servidor | Diagnóstico com logs em tempo real | `docs/screenshots/server-test.png` |

## 🛠️ Desenvolvimento

### Pré-requisitos

- Windows 10 ou 11
- [Node.js](https://nodejs.org/) com npm
- [Rust](https://www.rust-lang.org/tools/install)
- [Dependências do Tauri para Windows](https://v2.tauri.app/start/prerequisites/), incluindo Microsoft C++ Build Tools e WebView2
- Project Zomboid instalado para utilizar todas as funcionalidades

### Executando localmente

```powershell
npm install
npm run tauri:dev
```

Para trabalhar somente na interface web:

```powershell
npm run dev
```

Para validar o frontend e gerar a aplicação desktop:

```powershell
npm run build
npm run tauri:build
```

Os artefatos compilados pelo Tauri são gerados em `src-tauri/target/release/`.

### Comandos disponíveis

| Comando | Descrição |
| --- | --- |
| `npm install` | Instala as dependências JavaScript |
| `npm run dev` | Inicia somente o frontend com Vite |
| `npm run build` | Compila e valida o frontend |
| `npm run tauri:dev` | Inicia o frontend e a aplicação desktop em desenvolvimento |
| `npm run tauri:build` | Gera o build desktop de produção |

## 🧱 Tecnologias

| Camada | Tecnologias |
| --- | --- |
| Interface | React 19, TypeScript e Vite 8 |
| Estilos | Tailwind CSS 4 |
| Componentes e ícones | Base UI, shadcn e Lucide React |
| Aplicativo desktop | Tauri 2 |
| Backend local | Rust |
| Downloads da Workshop | SteamCMD |

<details>
<summary><strong>📂 Estrutura do projeto</strong></summary>
<br>

```text
.
├── resources/          # Arquivos de exemplo e SteamCMD empacotado
├── src/                # Interface React, componentes, tipos e integração Tauri
├── src-tauri/          # Backend Rust e configuração do aplicativo desktop
├── CONTRIBUTING.md     # Guia resumido de contribuição
├── package.json        # Dependências e scripts do frontend
└── README.md           # Documentação principal
```

</details>

<details>
<summary><strong>⚙️ Como funciona</strong></summary>
<br>

O frontend React chama comandos Tauri por meio de `invoke`. O backend Rust executa operações locais, como leitura e escrita de configurações, varredura de mods, download de itens da Workshop e teste de inicialização.

| Local | Uso |
| --- | --- |
| `Zomboid/Server` | Perfis de servidor e arquivos `.ini` |
| `Zomboid/mods` | Mods instalados localmente |
| Bibliotecas Steam | Itens baixados da Workshop do Project Zomboid |
| Diretórios personalizados | Pastas adicionais configuradas pelo usuário |
| Diretório de configuração do app | Caminhos salvos, RAM e locais adicionais de mods |

Ao atualizar os mods ativos de um servidor, o aplicativo escreve os campos `Mods` e `WorkshopItems` no arquivo `.ini`. O Workshop ID também é preservado ao copiar um mod para a biblioteca local.

</details>

## 🤝 Contribuindo

Contribuições são bem-vindas. Leia o [guia de contribuição](CONTRIBUTING.md) antes de enviar mudanças e mantenha os commits pequenos e objetivos.

O projeto segue o padrão **Conventional Commits**:

```text
<tipo>(<escopo>): <resumo curto>
```

Exemplos:

```text
feat(server): detectar instâncias automaticamente
fix(scanner): corrigir leitura de workshop id
chore: atualizar dependências de desenvolvimento
```

## 🚧 Estado atual

Este projeto está em desenvolvimento ativo. O foco atual é Windows, as capturas de tela oficiais ainda serão adicionadas e ainda não há uma suíte de testes automatizados configurada no repositório.

O status dos servidores listados começa como offline; o diagnóstico detalhado acontece ao executar o teste do servidor.

## 📄 Licença

Este repositório ainda não possui um arquivo de licença. Antes de reutilizar ou redistribuir o código, confirme os termos aplicáveis com o autor do projeto.
