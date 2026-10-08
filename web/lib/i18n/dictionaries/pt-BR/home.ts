import type { HomeDict } from "../types";

/** Brazilian Portuguese home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: construa com seus modelos e ferramentas",
  metaDescription:
    "Crie aplicativos, automatize fluxos de trabalho e use ferramentas conectadas com Codewhale. Código aberto, com suas APIs de modelos ou inferência local e auto-hospedada.",
  heroTitle: "Construa o que você tem em mente.",
  heroIntro:
    "Crie um aplicativo, automatize um fluxo de trabalho ou transforme uma pesquisa em algo útil. {brand} usa as APIs de modelos que você já tem, sua própria inferência e as ferramentas que conectar.",
  getCodewhale: "Instalar o Codewhale",
  heroInstallAria: "Comando de instalação",
  exploreProduct: "Explore o Codewhale",
  shotPreview: "Prévia do terminal",
  screenshotAlt:
    "Codewhale v{version}: captura do terminal com conversa, campo de mensagem e controles da sessão.",
  latestRelease: "Último lançamento {tag}",
  releaseUnavailable: "Status do lançamento indisponível",
  currentSource: "Código-fonte",
  sourceCandidate: "Não publicado",
  publishedRelease: "publicado",
  gainHeading: "Crie algo útil.",
  gainLede:
    "Comece pelo resultado que deseja. Codewhale trabalha com arquivos, comandos e ferramentas conectadas; você escolhe os acessos e as aprovações.",
  gain: [
    [
      "Crie aplicativos e ferramentas",
      "Transforme uma ideia em um aplicativo que funciona, um script útil ou um recurso em um projeto existente. Deixe o agente escrever, executar e testar com você."
    ],
    [
      "Automatize o trabalho repetitivo",
      "Transforme uma tarefa recorrente em um fluxo para terminal, scripts ou CI. Use um Fleet de agentes quando o trabalho puder acontecer em paralelo."
    ],
    [
      "Conecte as ferramentas que você usa",
      "Conecte ferramentas como Gmail e Slack por plugins, servidores MCP ou APIs. Trabalhe nesses serviços junto com seus arquivos e comandos."
    ]
  ],
  exampleTasks: [
    "Crie um aplicativo para agendar horários.",
    "Transforme um CSV de vendas em um relatório semanal que eu possa gerar novamente.",
    "Transforme as mensagens do meu e-mail conectado em uma lista de tarefas.",
  ],
  // A static example report built from local sample orders.
  reportTitle: "Relatório semanal de vendas",
  reportSampleLabel: "Relatório de exemplo · dados de amostra",
  reportDescription: "Peça ao Codewhale para agrupar os pedidos por semana e salvar o processo para o próximo CSV.",
  reportSourceLabel: "Dados de entrada:",
  reportColumns: ["Início da semana","Pedidos","Vendas (USD)"],
  reportTotalLabel: "Total",
  reportTrend: "Variação das vendas, da primeira semana à última: {change}.",
  reportDownloadLabel: "Baixar relatório CSV",
  chapterModels: "Seus modelos",
  modelsHeading: "Continue usando seus modelos.",
  modelsBody:
    "Conecte as APIs de modelos que já paga, use um gateway compatível ou execute inferência no seu hardware. Escolha um modelo por sessão e para cada agente de um Fleet.",
  modelsFacts: [
    [
      "Suas contas de API",
      "Conecte OpenAI, Anthropic, Google ou DeepSeek com suas próprias chaves."
    ],
    [
      "Seu gateway",
      "Use um endpoint compatível com OpenAI e escolha os modelos que ele oferece."
    ],
    [
      "Sua inferência",
      "Execute modelos locais ou auto-hospedados com Ollama, vLLM ou SGLang."
    ]
  ],
  modelsLink: "Ver modelos e provedores",
  startHeading: "Traga uma tarefa. Comece.",
  startLede:
    "Instale Codewhale, conecte um modelo e dê a ele algo que valha a pena fazer. Comece com um agente; adicione ferramentas ou uma equipe quando precisar.",
  startGuideLink: "Seguir o guia de primeiros passos",
  startVocabularyLink: "Ver o vocabulário do produto",
  chapterAvailability: "Onde funciona",
  availabilityHeading: "Comece pelo terminal.",
  availabilityLede:
    "O terminal e o cliente de navegador local já estão disponíveis. O aplicativo desktop nativo e o novo aplicativo web hospedado estão em desenvolvimento.",
  availability: [
    [
      "Terminal e navegador local",
      "Lançado",
      "Instale no Linux, macOS ou Windows e depois execute codewhale, ou codewhale web para o cliente de navegador local. npm e Cargo também funcionam; Android no Termux é uma prévia."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Disponível",
      "Um projeto separado, mantido pela comunidade: chat, threads e alterações de arquivos em uma barra lateral do VS Code sobre o mesmo Codewhale Runtime. Instale pelo VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Aplicativo web hospedado",
      "Prévia de desenvolvimento",
      "Está sendo reconstruído para acompanhar o aplicativo desktop. Hoje você pode entrar na conta e depois digitar /rc em uma sessão de terminal em execução para continuá-la na web; a execução de tarefas hospedadas ainda está sendo qualificada."
    ],
    [
      "Desktop",
      "Build de desenvolvimento",
      "O aplicativo nativo que está se tornando o cliente principal do Codewhale: pastas, conversas e conexões de modelos em uma única janela. Ainda não há download público."
    ],
    [
      "Computadores na nuvem",
      "Em desenvolvimento",
      "Computadores hospedados que executam suas tarefas."
    ]
  ],
  availabilityNote:
    "O terminal, o navegador local e a GUI não precisam de uma conta do Codewhale. A web hospedada e o aplicativo desktop usam uma conta. Se você usar sua própria chave de provedor, esse provedor cobra pelo uso.",
  accountLink: "Criar uma conta",
  surfacesHeading: "Uma tarefa. Seus arquivos, aplicativos e agentes.",
  surfaces: [
    [
      "Arquivos e terminal",
      "Crie arquivos, execute comandos, examine dados e teste o que constrói. Você define a pasta de trabalho e as permissões."
    ],
    [
      "Plugins e aplicativos conectados",
      "Adicione habilidades e ferramentas por plugins e MCP. Revise e habilite as conexões que deseja que o agente use."
    ],
    [
      "Navegador e computador · prévia",
      "Use ferramentas de navegador e o plugin Computer Use em aplicativos e sites, com o acesso que conceder."
    ],
    [
      "Sessões que você pode retomar",
      "Mantenha conversa, resultados das ferramentas e histórico juntos. Retome a tarefa no terminal ou no cliente de navegador local."
    ],
    [
      "Equipes de agentes",
      "Use Fleet para dividir um trabalho maior entre agentes com funções e modelos diferentes e acompanhar seu progresso em um só lugar."
    ]
  ],
  runtimeLink: "Ver todas as integrações",
  installBandHeading: "Instale no macOS ou Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binários",
  chinaMirrors: "Espelhos da China",
  installGuideLink: "Ler o guia de instalação",
  communityHeading: "Faça o Codewhale do seu jeito.",
  communityBody:
    "Codewhale é de código aberto. Leia o código, crie um plugin, compartilhe um fluxo de trabalho ou ajude a melhorar a próxima versão.",
  communityLinksAria: "Links da comunidade",
  contribute: "Contribua no GitHub",
};
