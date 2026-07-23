# Changelog

Histórico das principais mudanças do **PZ Manager / Project Zomboid Mod Manager**.

As datas seguem o formato `AAAA-MM-DD`. Este arquivo resume o que foi adicionado,
melhorado e corrigido em cada release pública.

## [Unreleased]

- Nenhuma mudança registrada ainda.

## [0.5.0] - 2026-07-20

Release focada em operação real de servidores, principalmente servidores Linux
remotos e listas grandes de mods da Workshop.

### Adicionado

- Banco comunitário de mapeamentos `Mod ID -> Workshop ID`.
- Sincronização dos mapeamentos da Workshop com cache local.
- Upload automático, em segundo plano, de mapeamentos descobertos localmente.
- Erro visual quando a sincronização do banco de mapeamentos falha.
- Ferramentas para editar Workshop ID direto nos detalhes do mod.
- Assistente para corrigir Workshop IDs ausentes em mods ativos do servidor.
- Modal remoto de inicialização com dois botões:
  - `Start server`;
  - `Start -nosteam`.
- Suporte remoto à flag `-nosteam` no helper Linux.
- Logs claros indicando quando o servidor foi iniciado com `-nosteam`.
- Testes unitários para geração de scripts de start normal e `-nosteam`.
- Visualizador de logs do servidor dentro do app.
- Preview de logs locais e remotos.
- Filtros e ações para copiar/atualizar logs.
- Controle remoto de startup com stream de saída em tempo real.
- Canal remoto para envio de comandos ao servidor via FIFO.
- Stop remoto com fluxo de `save` e `quit`.
- Upload de mods locais selecionados para servidor remoto.
- Deploy de perfil local para servidor Linux remoto com seus mods ativos.

### Melhorado

- Mapeamentos remotos e locais agora são mesclados em vez de substituir dados bons
  por respostas parciais.
- O app continua usando o cache local quando o serviço de mapeamentos está fora do ar.
- `Mods=` e `WorkshopItems=` ficaram mais seguros para listas ativas com IDs
  corrigidos.
- Preflight de testes ficou mais rigoroso para dependências, ordem de carregamento,
  compatibilidade B41/B42 e conflitos de porta.
- Fluxo remoto reaproveita firewall, logs, status, comandos e parada entre start
  normal e start `-nosteam`.
- CSP do Tauri foi ajustada para permitir o serviço HTTP configurado de
  mapeamentos da Workshop.
- Documentação principal, documentação em português e resumo do projeto foram
  atualizados para refletir a versão `0.5.0`.

### Corrigido

- Falha visual falsa ao sincronizar IDs da Workshop quando o endpoint retornava
  dados válidos em formato não tratado pelo app.
- Perda de mapeamentos locais quando o app fazia upload automático de itens ainda
  não cadastrados na API.
- Duplicação da flag `-nosteam` quando o launcher original já continha a opção.
- Warning do `cargo clippy` relacionado à ordenação de logs.
- Dependência de runtime desnecessária no pacote npm de produção.

### Validação

- `npm run build`
- `npm audit --omit=dev`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`

## [0.4.0] - 2026-07-05

Release focada em workspaces remotos Linux, compatibilidade multiplataforma e
preparação de builds para Windows e Linux.

### Adicionado

- Seletor de workspace ao abrir o app.
- Workspace local para o fluxo Windows existente.
- Workspace remoto para servidores Linux via SSH.
- Perfis de conexão SSH salvos.
- Teste de conectividade antes de entrar no workspace remoto.
- Helper Linux para executar operações remotas.
- Telas de setup remoto.
- Suporte inicial a ciclo de vida remoto do servidor.
- Envio de comandos para servidor remoto em execução.
- Stream de logs de servidores remotos.
- Cache de mods e servidores remotos.
- Suporte a builds Linux desktop.
- Workflows de release no GitHub Actions para gerar assets Windows e Linux.
- Bundle do helper Linux `pzmm-helper-linux-x86_64`.
- Localização completa das telas de workspace, SSH e guias de OpenSSH.
- Exclusão segura de mods locais ou remotos pela biblioteca.
- Métricas de dashboard para servidores remotos, incluindo ping e jogadores no
  formato `X/Y`.

### Melhorado

- Erros de SSH passaram a mostrar mensagens amigáveis e traduzidas.
- A varredura de pastas foi otimizada para respeitar caminhos configurados.
- Workspaces locais não exibem métricas remotas desnecessárias.
- O dashboard remoto passou a exibir contadores e ícones mais claros.
- Compatibilidade entre runners Windows e Linux no CI foi ampliada.

### Observações

- O suporte remoto Linux ainda era experimental nesta versão.
- O foco inicial era Ubuntu/Debian com `systemd`.
- Servidores Windows remotos deveriam ser acessados via RDP, executando o app
  localmente dentro da VM.

## [0.3.0] - 2026-06-16

Release focada em performance da biblioteca de mods e redução de varreduras
repetidas.

### Adicionado

- Cache persistente de backend para a biblioteca de mods.
- Reuso da biblioteca em cache durante validações de preflight.
- Ação de rescan completo da biblioteca, limpando cache frontend e backend.
- Revalidação rápida do cache baseada em arquivos relevantes dos pacotes.

### Melhorado

- A listagem de mods ficou mais rápida em bibliotecas grandes.
- A tela de Configurações passou a hidratar dados a partir das últimas
  configurações conhecidas, reduzindo flicker.
- Imagens locais de mods passaram a carregar corretamente pelo protocolo de assets
  do Tauri.
- Compatibilidade dos checks de `cargo clippy` no CI foi melhorada.
- Metadados de release e changelog foram preparados para a versão `0.3.0`.

### Corrigido

- Exibição incorreta da quantidade de downloads simultâneos do SteamCMD nas
  Configurações.
- Carregamento de imagens locais que havia quebrado após otimizações na listagem
  de mods.

## [0.2.0] - 2026-06-14

Release focada em edição de configuração de servidor dentro do app.

### Adicionado

- Modal de configuração de servidor.
- Edição de opções comuns do arquivo `.ini`.
- Edição de `SandboxVars.lua` em aba dedicada.
- Opções legíveis para valores de Sandbox, evitando edição por números brutos.
- Badge visual para indicar valor padrão de opções Sandbox.
- Agrupamento de configurações Sandbox por seções do jogo base e seções de mods.
- Melhor suporte ao manuseio de arquivos `.lua` relacionados ao perfil.

### Melhorado

- Interface de criação de servidor foi renovada.
- Fluxo de configuração ficou menos dependente de edição manual de arquivos.
- Perfil de servidor passou a ser mais fácil de revisar antes dos testes.

### Mantido

- Criação e gerenciamento de perfis.
- Suporte a Build 41 e Build 42.
- Ativação, desativação e reordenação de mods.
- Atualização automática de `.ini`.
- Detecção de dependências ausentes e ordem inválida.
- Downloads via SteamCMD.
- Diagnóstico de startup com logs em tempo real.
- Seleção de idioma entre inglês, português brasileiro e automático.

## [0.1.0] - 2026-06-01

Primeira release pública do PZ Manager.

### Adicionado

- Criação e gerenciamento de perfis de servidores Project Zomboid.
- Suporte inicial a Build 41 e Build 42.
- Ativação, desativação e reordenação de mods ativos.
- Escrita automática do arquivo `.ini` do servidor.
- Detecção de dependências ausentes.
- Validação de ordem de carregamento.
- Leitura de mods em pastas locais, Steam Workshop e diretórios personalizados.
- Downloads de itens individuais ou coleções públicas da Workshop via SteamCMD.
- Diagnóstico de inicialização do servidor com logs em tempo real.
- Checagem de portas configuradas antes de testar servidor.
- Idiomas inglês, português brasileiro e detecção automática.

### Observações

- O projeto era focado principalmente em Windows.
- Algumas funcionalidades ainda estavam em fase inicial e sujeitas a mudanças.

## Links

- [0.5.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.5.0)
- [0.4.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.4.0)
- [0.3.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.3.0)
- [0.2.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.2.0)
- [0.1.0](https://github.com/WiliamMelo01/ProjectZomboidModManager/releases/tag/v0.1.0)
