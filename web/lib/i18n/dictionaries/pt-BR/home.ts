import type { HomeDict } from "../types";

/** Brazilian Portuguese home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: construa com seus modelos e ferramentas",
  metaDescription:
    "Crie aplicativos, automatize fluxos de trabalho e use ferramentas conectadas com Codewhale. Código aberto, com suas APIs de modelos ou inferência local e auto-hospedada.",
  heroTitle: "Crie aplicativos e automatize seu trabalho.",
  heroIntro:
    "{brand} é um agente de código aberto que escreve código, executa comandos e trabalha com as ferramentas que você conecta. Use as APIs de modelos que você já utiliza ou execute modelos localmente e nos seus próprios servidores.",
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
  gainHeading: "O que você pode fazer",
  gainLede:
    "Descreva o que você quer criar ou automatizar. Codewhale pode editar arquivos, executar comandos e verificar o resultado, com acesso controlado por você.",
  gain: [
    [
      "Crie aplicativos e ferramentas",
      "Crie um aplicativo, adicione uma funcionalidade ou escreva um script. Codewhale pode trabalhar com os arquivos do projeto, executar o código e testar o que cria."
    ],
    [
      "Automatize o trabalho repetitivo",
      "Execute fluxos de trabalho pelo terminal, scripts ou CI. Para tarefas maiores, delegue partes do trabalho a uma Fleet de agentes com modelos diferentes."
    ],
    [
      "Conecte as ferramentas que você usa",
      "Adicione ferramentas por meio de plugins e servidores MCP ou use APIs nos seus próprios scripts. Cada serviço precisa de configuração e autenticação próprias."
    ]
  ],
  chapterModels: "Seus modelos",
  modelsHeading: "Use os modelos que você escolher",
  modelsBody:
    "Conecte suas contas de provedores, um endpoint compatível com OpenAI ou modelos locais e hospedados por você. Escolha um modelo para a sessão e para cada agente de uma Fleet.",
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
  startHeading: "Comece a usar",
  startLede:
    "Instale Codewhale, conecte um modelo e abra uma pasta de projeto. Você pode adicionar plugins e mais agentes conforme precisar.",
  startGuideLink: "Seguir o guia de primeiros passos",
  startVocabularyLink: "Ver o vocabulário do produto",
  chapterAvailability: "Onde funciona",
  availabilityHeading: "Disponível agora e em desenvolvimento",
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
  surfacesHeading: "Trabalhe com arquivos e ferramentas",
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
  runtimeLink: "Explore ferramentas e integrações",
  installBandHeading: "Instale no macOS ou Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binários",
  chinaMirrors: "Espelhos da China",
  installGuideLink: "Ler o guia de instalação",
  communityHeading: "Contribua com Codewhale",
  communityBody:
    "Relate um bug, melhore a documentação ou contribua com código no GitHub. Você também pode criar plugins e compartilhar fluxos de trabalho com outros usuários.",
  communityLinksAria: "Links da comunidade",
  contribute: "Contribua no GitHub",
};
