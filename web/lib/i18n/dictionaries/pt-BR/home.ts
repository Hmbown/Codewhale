import type { HomeDict } from "../types";

/**
 * Brazilian Portuguese home dictionary — native copy for the whale-road
 * landing page. Translates the English reference in en/home.ts: an
 * open-source coding agent for any model, control you can check, and
 * availability stated per surface as it is today. Product vocabulary stays
 * literal (Plan / Work / Operate, Ask / Auto-Review / Full Access,
 * Codewhale, codewhale exec, Fleet, MCP, Runtime, /receipts).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: o agente de programação de código aberto para qualquer modelo",
  metaDescription:
    "O Codewhale é um agente de programação de código aberto para o seu terminal. Ele lê seu projeto, edita arquivos e executa seus testes com o modelo hospedado ou local que você escolher.",
  heroTitle: "O agente de programação de código aberto para qualquer modelo",
  heroIntro:
    "{brand} lê seu projeto, edita arquivos e executa seus testes pelo terminal. Conecte um modelo hospedado ou local e escolha quais ações precisam da sua aprovação.",
  getCodewhale: "Instalar o Codewhale",
  heroInstallAria: "Comando de instalação",
  exploreProduct: "Ver como funciona",
  shotPreview: "Prévia do terminal",
  shotBuild: "build de desenvolvimento v{version}",
  screenshotAlt:
    "Codewhale v{version}, versão de desenvolvimento: baleia, nova sessão, campo de mensagem, permissões Ask, modo Work e estado do modelo. Renderização da saída real de um terminal isolado.",
  latestRelease: "Último lançamento {tag}",
  releaseUnavailable: "Status do lançamento indisponível",
  currentSource: "Código-fonte",
  sourceCandidate: "Não publicado",
  publishedRelease: "publicado",
  figcaptionSourceCandidate: "não publicado",
  chapterTerminal: "Seu terminal",
  chapterTerminalTitle: "Acompanhe cada edição e cada comando enquanto são executados",
  gainHeading: "Delegue a tarefa e mantenha o controle",
  gainLede:
    "Peça um resultado: corrigir um bug, explicar um módulo ou automatizar uma tarefa que você repete. Comece com um agente e adicione mais agentes quando o trabalho crescer.",
  gain: [
    [
      "Altere o código e confira",
      "O agente inspeciona seu projeto, edita arquivos e executa seus testes. Acompanhe cada edição e cada resultado de comando enquanto ele trabalha."
    ],
    [
      "Automatize o trabalho repetitivo",
      "Execute codewhale exec a partir de scripts e CI. Use um Fleet para dividir um trabalho maior entre vários agentes."
    ],
    [
      "Mantenha o controle",
      "Defina as permissões antes de começar, responda aos pedidos de aprovação e pare uma tarefa a qualquer momento. Execute /receipts para listar cada arquivo, comando e aprovação de uma sessão."
    ]
  ],
  chapterModels: "Seus modelos",
  modelsHeading: "Escolha um modelo para cada tarefa",
  modelsBody:
    "Escolha, para cada sessão, um provedor integrado, qualquer endpoint compatível com OpenAI ou um modelo local. Sua conexão de modelo fica separada de qualquer conta do Codewhale.",
  modelsFacts: [
    ["Hospedado", "Sua própria chave de API, salva com codewhale auth set --provider <id>"],
    ["Gateway", "Um endpoint para muitos modelos; você continua escolhendo o provedor"],
    ["Local", "vLLM, SGLang ou Ollama em localhost, normalmente sem chave"],
  ],
  modelsLink: "Ver modelos e provedores",
  startHeading: "Instale, conecte um modelo e execute uma tarefa",
  startLede:
    "Execute sua primeira tarefa em três passos a partir da pasta do seu projeto. Adicione um Fleet depois, se o trabalho precisar de vários agentes.",
  startGuideLink: "Seguir o guia de primeiros passos",
  startVocabularyLink: "Ver o vocabulário do produto",
  chapterAvailability: "Onde funciona",
  availabilityHeading: "Use no seu terminal hoje",
  availabilityLede:
    "Use agora o terminal, o cliente de navegador local ou a CodeWhale GUI da comunidade. O aplicativo desktop e o aplicativo web hospedado reconstruído estão em desenvolvimento e compartilham o mesmo modelo de sessão.",
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
    "O terminal, o navegador local e a GUI não precisam de uma conta do Codewhale. A web hospedada e o desktop usam uma conta, que não substitui sua conexão de modelo; seu provedor cobra o uso feito com sua própria chave.",
  accountLink: "Criar uma conta",
  surfacesHeading: "Amplie o alcance do agente",
  surfaces: [
    ["Arquivos e comandos", "Leia o projeto, edite arquivos, execute testes e inspecione a saída dentro das permissões que você definir."],
    ["Plugins e MCP", "Conecte mais ferramentas e serviços. Cada plugin fica desativado até você revisá-lo e ativá-lo."],
    ["Computer Use · prévia", "Um plugin que permite ao agente ver e operar outros aplicativos. Você o ativa e concede as permissões de sistema que ele solicitar."],
    ["Sessões salvas", "Mantenha a conversa e os resultados das ferramentas juntos e retome o trabalho em vez de começar do zero. O navegador local abre a mesma sessão no seu computador."],
    ["Fleet", "Atribua partes de uma tarefa a agentes com modelos e papéis diferentes e depois acompanhe o progresso deles."],
  ],
  runtimeLink: "Ver todas as integrações",
  installBandHeading: "Instale no macOS ou Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binários",
  chinaMirrors: "Espelhos da China",
  installGuideLink: "Ler o guia de instalação",
  communityHeading: "Construa o Codewhale com a gente",
  communityBody:
    "Relate um bug, proponha uma funcionalidade ou envie seu primeiro pull request no GitHub. Correções pequenas e testadas são bem-vindas.",
  communityLinksAria: "Links da comunidade",
  contribute: "Enviar um pull request",
};
