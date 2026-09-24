<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale é um agente de código aberto que lê seu projeto, edita arquivos, executa comandos e verifica o próprio trabalho usando um modelo hospedado ou local à sua escolha. Comece com uma tarefa no terminal. Para um trabalho maior, distribua partes do trabalho entre agentes com diferentes modelos e funções.

![Codewhale em execução em um terminal](web/public/codewhale-tui-d7a9a1c.png)

*Prévia do terminal em uma build de desenvolvimento da v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Instalação

Para uma nova instalação no macOS ou Linux, use a versão oficial do GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

O instalador seleciona a versão publicada mais recente. O [histórico de alterações](CHANGELOG.md) também descreve a versão candidata ainda não publicada da próxima versão; essas alterações só são incluídas nos downloads publicados quando a versão estiver disponível.

No Windows, baixe o instalador ou arquivo correspondente em [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Para atualizar uma instalação direta existente, execute `codewhale update`, ou `codewhale update --check` apenas para verificar. O atualizador mostra o caminho do executável e preserva builds mais recentes. npm e Cargo são opções secundárias; consulte o [guia de instalação](docs/INSTALL.md) para migrar de um gerenciador de pacotes e configurar PATH.

A primeira execução abre direto no compositor; não há um assistente de configuração. As respostas do modelo exigem um modelo hospedado ou local conectado: até que haja um, a tela inicial mostra "no model connected". Execute `/provider` (ou pressione F3) para adicionar uma chave hospedada ou escolher um runtime local. Se o Ollama já estiver em execução com um modelo de chat, o Codewhale muda para ele sozinho. O Codewhale também oferece suporte a npm e Cargo como opções secundárias de distribuição, além de Docker, Nix, Scoop, Android/Termux e um espelho CNB opcional. Instalações existentes feitas por gerenciadores de pacotes recebem instruções de migração. Consulte a [ajuda de instalação e PATH](docs/INSTALL.md).

O preenchimento automático com Tab é ativado com um comando por shell — `codewhale completion bash|zsh|fish|powershell|elvish`. Consulte o [preenchimento automático do shell](docs/INSTALL.md#8-shell-completions).

## Uso

Abra um terminal na pasta do seu projeto e execute `codewhale`. Escolha seu provedor com `/provider` e seu modelo com `/model`. Depois, descreva uma tarefa concreta:

```text
Fix the failing tests and explain what changed.
```

Ou execute uma tarefa sem abrir a TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

O Codewhale pode ler seu repositório, editar arquivos, executar comandos, verificar resultados e continuar trabalhando em direção a um objetivo. Use `/mode plan` para explorar sem alterar arquivos nem executar comandos de shell, e `/mode work` quando quiser que ele faça alterações. Pressione `Shift+Tab` para escolher Ask, Auto-Review ou Full Access; o [guia de modos e permissões](docs/MODES.md) explica o que cada opção permite.

## Terminal, aplicativos e Computer Use

O terminal e os clientes gráficos se conectam ao Runtime do Codewhale, que executa o agente e suas ferramentas:

- **Terminal:** `codewhale` abre a interface interativa; `codewhale exec` executa uma tarefa a partir de um script ou de um job de CI.
- **Navegador local:** `codewhale web` abre o [cliente web local](docs/WEB.md) incluído, que usa o mesmo runtime.
- **Aplicativo de desktop Codewhale (GPUI):** um aplicativo de desktop nativo, desenvolvido em um repositório separado, é a direção do cliente do produto com login. O aplicativo web hospedado em app.codewhale.net será reconstruído à sua imagem; o site de marketing, o login, a cobrança e as páginas legais e de download permanecem na web. A disponibilidade é informada na [página do produto](https://codewhale.net/en/product).

**Computer Use adiciona ferramentas para observar outros aplicativos e interagir com eles.** O plugin está incluído no código-fonte atual. Revise o acesso solicitado e habilite-o antes de usar; as permissões do sistema operacional e os requisitos da plataforma continuam sendo necessários. Consulte o [guia de Computer Use](crates/tui/plugins/computer-use/README.md) incluído e a [configuração de plugins](docs/PLUGINS.md).

Para VS Code, a extensão CodeWhale mantida pela comunidade se conecta ao Runtime local por uma barra lateral. Instale pelo [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); o código-fonte está no [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Por que usar o Codewhale

- **Escolha seus modelos.** Conecte provedores hospedados ou modelos locais por meio do Ollama, vLLM ou SGLang. Use `/provider` para trocar de provedor e `/model` para escolher um modelo.
- **Mantenha o controle.** Revise as ações propostas e as alterações resultantes nos arquivos. As configurações de aprovação determinam quando uma revisão é necessária; Full Access continua respeitando os limites obrigatórios das políticas. `/undo` e `/restore` ajudam a recuperar alterações no espaço de trabalho.
- **Mantenha trabalhos longos organizados.** Salve sessões, defina um `/goal` duradouro, revise os fluxos de trabalho antes da execução e coordene agentes sem transformar as instruções internas deles em parte da sua conversa.
- **Amplie o agente que você já tem.** Conecte servidores MCP e habilidades, configure hooks e mantenha as funções dos agentes como arquivos legíveis no projeto ou nas suas configurações pessoais.

Execute `/help` na TUI para ver comandos e atalhos de teclado.

## Segurança

O Codewhale é executado na sua máquina com o acesso que você conceder. Os modos de aprovação e as regras do repositório limitam o que o agente pode fazer; o sandbox opcional do sistema operacional adiciona um limite de execução mais forte quando disponível. Preços de modelos desconhecidos continuam sendo mostrados como desconhecidos, em vez de serem informados como gratuitos.

Leia a [ordem de autorização](docs/AUTHORIZATION_ORDER.md) para conhecer a hierarquia exata das políticas e a [configuração](docs/CONFIGURATION.md) para os ajustes locais.

## Documentação

- [Provedores e modelos locais](docs/PROVIDERS.md)
- [Equipes de agentes](docs/FLEET.md)
- [MCP](docs/MCP.md), [hooks](docs/HOOKS.md) e [configuração](docs/CONFIGURATION.md)
- [Cliente web local](docs/WEB.md)
- [Toda a documentação](docs/README.md)
- [Estrutura do repositório e guia de contribuição](CONTRIBUTING.md#project-structure)

## Participe da comunidade

**Relatos de bugs, ideias de funcionalidades e pull requests são bem-vindos**, tanto de quem usa o Codewhale há meses quanto de quem está experimentando pela primeira vez. Se estiver faltando um provedor, se um fluxo de trabalho for inconveniente ou se a interface do terminal atrapalhar, [abra uma issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) ou [envie um pull request](CONTRIBUTING.md) para melhorarmos juntos. Primeiras contribuições são bem-vindas, e os contribuidores mantêm o crédito pelo trabalho incorporado ao projeto.

Participe do [Discord](https://discord.gg/37gfS3ksug), ou adicione Hunter no WeChat (`hunterbown`) e peça para entrar no grupo Whale Brothers.

## História do projeto

O Codewhale começou como `deepseek-tui` e ainda preserva a compatibilidade com as configurações e sessões desse projeto. Hoje ele é neutro em relação a provedores, mantido de forma independente e não tem afiliação com nenhum provedor de modelos.

Agradecemos a cada contribuidor e às comunidades de código aberto que ajudaram o projeto a crescer. Consulte o [registro de contribuidores](docs/CONTRIBUTORS.md).

## Licença

[MIT](LICENSE). As partes adaptadas de outros projetos de código aberto estão registradas nos [avisos de terceiros](docs/THIRD_PARTY_NOTICES.md).
