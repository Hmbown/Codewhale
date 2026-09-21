import type { HomeDict } from "../types";

/**
 * Brazilian Portuguese home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Crie e automatize com os modelos que você escolher",
  metaDescription:
    "Crie software, trabalhe com seus arquivos e automatize as tarefas do dia a dia com agentes de código aberto e os modelos de IA hospedados ou locais que você escolher.",
  heroTitle: "Crie e automatize com os modelos que você escolher",
  heroIntro:
    "{brand} oferece agentes que podem criar software, trabalhar com seus arquivos e transformar tarefas repetitivas em fluxos de trabalho reutilizáveis. Diga a eles o que você quer realizar e escolha os modelos hospedados ou locais adequados ao trabalho, com a liberdade de trocar de provedor ao longo do caminho.",
  getCodewhale: "Obter o Codewhale",
  exploreProduct: "Explorar o produto",
  shotPreview: "Prévia do terminal",
  shotBuild: "build de desenvolvimento v{version}",
  screenshotAlt:
    "Codewhale v{version}, versão de desenvolvimento: baleia, nova sessão, campo de mensagem, permissões Ask, modo Work e estado do modelo. Renderização da saída real de um terminal isolado.",
  latestRelease: "Último lançamento {tag}",
  releaseUnavailable: "Status do lançamento indisponível",
  currentSource: "Código-fonte",
  sourceCandidate: "Não publicado",
  providerRoutes: "{count} provedores",
  publishedRelease: "publicado",
  figcaptionSourceCandidate: "não publicado",
  chapterTerminal: "Seu terminal",
  chapterTerminalTitle: "Comece com algo que você queira criar",
  gainHeading:
    "O que você pode fazer com o Codewhale",
  gainLede:
    "Comece com um projeto, uma pergunta ou uma tarefa que você queira automatizar, depois trabalhe com um agente ou distribua partes de um trabalho maior entre vários.",
  gain: [
    [
      "Crie algo",
      "Descreva o que você quer criar e trabalhe com agentes que podem ler seu código, editar arquivos, executar comandos e conferir o resultado."
    ],
    [
      "Automatize o trabalho do dia a dia",
      "Crie scripts e fluxos de trabalho para as tarefas que você repete, para poder executá-los novamente pelo terminal sempre que precisar."
    ],
    [
      "Trabalhe com modelos diferentes",
      "Use modelos hospedados ou locais para seus agentes, com modelos e papéis diferentes cuidando das partes do trabalho às quais são adequados."
    ]
  ],
  chapterModels: "Seus modelos",
  modelsHeading: "Opções de modelos para cada tarefa",
  modelsBody:
    "Conecte-se diretamente a um provedor de modelos hospedados, use um gateway para acessar vários provedores ou execute um modelo localmente, depois escolha qual modelo cada sessão usa enquanto trabalha.",
  modelsFacts: [
    ["Hospedado", "Sua própria chave de API, salva com codewhale auth set --provider <id>"],
    ["Gateway", "Um endpoint para muitos modelos, o provedor continua sendo escolha sua"],
    ["Local", "vLLM, SGLang, Ollama em localhost — normalmente sem chave"],
  ],
  modelsLink: "Explorar modelos e provedores",
  startHeading: "Primeiros passos com o Codewhale",
  startLede:
    "Depois de instalar o Codewhale e conectar um modelo, você pode descrever sua primeira tarefa no terminal e adicionar um Fleet quando quiser dividir o trabalho entre vários agentes.",
  startGuideLink: "Ler o guia de primeiros passos",
  startVocabularyLink: "Ver o vocabulário do produto",
  chapterAccount: "Obter o Codewhale",
  availabilityHeading: "Onde você pode usar o Codewhale",
  availabilityLede:
    "Você já pode usar o Codewhale no seu terminal enquanto desenvolvemos o aplicativo web, o aplicativo desktop e os computadores na nuvem.",
  availability: [
    [
      "Terminal",
      "Lançado",
      "Binários das versões publicadas no GitHub para Linux, macOS e Windows; npm e Cargo são alternativas. Android no Termux é uma prévia."
    ],
    [
      "Aplicativo web",
      "Prévia de desenvolvimento",
      "Acesso à conta e pareamento com o navegador na prévia de desenvolvimento."
    ],
    [
      "Desktop",
      "Build de desenvolvimento",
      "O aplicativo para macOS está em desenvolvimento; o download público virá mais adiante."
    ],
    [
      "Computadores na nuvem",
      "Em desenvolvimento",
      "Computadores hospedados para executar suas tarefas."
    ]
  ],
  availabilityNote:
    "Você pode usar o terminal sem uma conta do Codewhale, e qualquer uso de modelos hospedados é cobrado pelo seu provedor.",
  accountLink: "Criar uma conta",
  surfacesHeading: "Formas de trabalhar com o Codewhale",
  surfaces: [
    ["TUI", "Trabalho interativo no terminal"],
    ["codewhale exec", "Scripts e CI"],
    ["Cliente web local","Interface em localhost; ambiente de trabalho web hospedado em desenvolvimento"],
    ["Runtime API + MCP", "Integrações locais"],
    ["Fleet","Vários agentes no mesmo trabalho"],
  ],
  runtimeLink: "Explorar integrações",
  installBandHeading: "Instale o Codewhale no macOS ou Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binários",
  chinaMirrors: "Espelhos da China",
  installGuideLink: "Ler o guia de instalação",
  communityHeading: "Ajude a melhorar o Codewhale",
  communityBody:
    "Se você encontrou um bug, tem uma ideia de funcionalidade ou quer enviar seu primeiro pull request, gostaríamos de ouvir você e trabalhar juntos nos próximos passos.",
  communityLinksAria: "Links da comunidade",
  contribute: "Enviar um pull request",
};
