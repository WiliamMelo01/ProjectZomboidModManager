<div align="center">

# PZ Manager

### Gerencie mods de servidores multiplayer de Project Zomboid sem editar configurações manualmente.

[![Versão](https://img.shields.io/badge/versão-0.0.1-6d5dfc?style=for-the-badge)](package.json)
![Plataforma](https://img.shields.io/badge/plataforma-Windows-0078D4?style=for-the-badge&logo=windows)
![Desktop](https://img.shields.io/badge/desktop-Tauri-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)
![Status](https://img.shields.io/badge/status-em%20desenvolvimento-F59E0B?style=for-the-badge)

</div>

---

## Sobre

O **PZ Manager** é um aplicativo desktop para organizar mods de servidores de **Project Zomboid**. Ele encontra perfis existentes, monta uma biblioteca a partir de mods locais e da Steam Workshop, atualiza arquivos `.ini` e executa um teste de inicialização com logs em tempo real.

O aplicativo suporta perfis para **Build 41** e **Build 42**. Cada servidor mantém sua própria build, lista de mods e itens da Workshop.

## Funcionalidades

| Recurso | O que você pode fazer |
| --- | --- |
| **Servidores** | Criar perfis, listar servidores existentes, pesquisar, ocultar perfis e clonar listas entre servidores da mesma build. |
| **B41 e B42** | Escolher a build por perfil, trocar a versão com confirmação e identificar mods incompatíveis. |
| **Mods ativos** | Ativar, desativar e reorganizar mods com atualização automática do arquivo `.ini`. |
| **Dependências** | Detectar dependências ausentes, instalar itens necessários e validar a ordem de carregamento. |
| **Biblioteca** | Encontrar mods locais, itens da Steam Workshop e mods armazenados em pastas personalizadas. |
| **Downloads** | Baixar mods individuais ou coleções completas usando SteamCMD com login anônimo. |
| **Diagnóstico** | Testar a inicialização do servidor, acompanhar logs e identificar conflitos de porta. |
| **Configurações** | Detectar Project Zomboid e SteamCMD, ajustar RAM, idioma e diretórios monitorados. |
| **Idiomas** | Usar inglês ou português brasileiro, com detecção automática e troca imediata. |

## Suporte B41 e B42

Perfis antigos sem metadados continuam abrindo como **B41**. Novos perfis permitem escolher entre `B41` e `B42`.

Na biblioteca, cada mod recebe badges de compatibilidade. Pacotes híbridos aparecem uma única vez mesmo quando possuem variantes para as duas builds.

O suporte à B42 preserva a estrutura versionada dos pacotes:

```text
mods/
└── ExampleMod/
    ├── common/
    ├── 42/
    │   └── mod.info
    └── 42.17/
        └── mod.info
```

Ao ativar mods:

- Perfis B41 escrevem o Mod ID tradicional em `Mods=`.
- Perfis B42 escrevem o ID da variante compatível.
- `WorkshopItems=` mantém Workshop IDs únicos.
- Mods incompatíveis continuam visíveis para remoção manual.
- O preflight do teste bloqueia dependências ausentes, ordem inválida e mods incompatíveis.

## Biblioteca e SteamCMD

O aplicativo lê mods instalados em `Zomboid/mods`, bibliotecas Steam e diretórios personalizados.

Ao trazer um mod para a pasta local, o pacote completo é copiado. Isso preserva variantes B41, diretórios versionados B42, conteúdo compartilhado em `common` e o marcador `.pzmm-workshop-id`.

Downloads aceitam Workshop ID numérico ou URL:

- Item individual ou coleção pública.
- Progresso item a item.
- Cancelamento durante o download.
- Nova tentativa somente para itens que falharam.
- Validação completa opcional para investigar arquivos corrompidos.
- Atualização automática da biblioteca ao finalizar.

## Teste do servidor

O diagnóstico executa uma inicialização controlada e exibe os logs em tempo real. Antes de iniciar, o aplicativo:

1. Valida mods ativos e dependências.
2. Verifica a ordem de carregamento.
3. Verifica compatibilidade com B41 ou B42.
4. Procura conflitos nas portas configuradas.

A B42 possui um timeout maior porque a inicialização pode levar mais tempo.

## Internacionalização

O idioma pode ser alterado em **Configurações**:

- `Automático`: usa `pt-BR` quando o sistema estiver em qualquer idioma `pt-*`; caso contrário usa inglês.
- `English`
- `Português (Brasil)`

A preferência é salva em `settings.ini` e aplicada imediatamente.

| Camada | Implementação |
| --- | --- |
| Frontend React | [`i18next`](https://www.i18next.com/) e [`react-i18next`](https://react.i18next.com/) |
| Backend Rust e menu nativo | [`rust-i18n`](https://docs.rs/rust-i18n/latest/rust_i18n/) |
| Catálogo frontend | `src/i18n/resources.ts` |
| Catálogo backend | `src-tauri/locales/app.yml` |

## Primeiros passos

1. Abra **Configurações** e confira se o SteamCMD foi localizado.
2. Verifique se o executável do Project Zomboid foi detectado.
3. Escolha o idioma desejado ou mantenha a detecção automática.
4. Adicione diretórios personalizados caso mantenha mods fora das pastas padrão.
5. Atualize a biblioteca.
6. Crie um servidor selecionando a build e os mods.
7. Revise dependências e execute um teste de inicialização.

## Desenvolvimento

### Pré-requisitos

- Windows 10 ou 11
- [Node.js](https://nodejs.org/) com npm
- [Rust](https://www.rust-lang.org/tools/install)
- [Dependências do Tauri para Windows](https://v2.tauri.app/start/prerequisites/)
- Project Zomboid instalado para utilizar todas as funcionalidades

### Executando localmente

```powershell
npm install
npm run tauri:dev
```

Para trabalhar somente na interface:

```powershell
npm run dev
```

Para gerar o build desktop:

```powershell
npm run tauri:build
```

### Validação

```powershell
npm run build
cd src-tauri
cargo test
cargo fmt --check
cd ..
git diff --check
```

## Tecnologias

| Camada | Tecnologias |
| --- | --- |
| Interface | React 19, TypeScript e Vite 8 |
| Estilos | Tailwind CSS 4 |
| Componentes e ícones | Base UI, shadcn e Lucide React |
| Aplicativo desktop | Tauri 2 |
| Backend local | Rust |
| Downloads da Workshop | SteamCMD |
| Internacionalização | i18next, react-i18next e rust-i18n |

## Estrutura do projeto

```text
.
├── resources/             # Arquivos de exemplo e recursos empacotados
├── src/                   # Interface React, componentes, tipos e catálogos frontend
├── src-tauri/
│   ├── locales/           # Catálogos rust-i18n do backend
│   └── src/               # Backend Rust e comandos Tauri
├── package.json           # Dependências e scripts do frontend
└── README.md              # Documentação principal
```

## Estado atual

O projeto está em desenvolvimento ativo e o foco atual é Windows. O status dos servidores listados começa como offline; o diagnóstico detalhado acontece ao executar o teste do servidor.

## Licença

Este repositório ainda não possui um arquivo de licença. Antes de reutilizar ou redistribuir o código, confirme os termos aplicáveis com o autor do projeto.
