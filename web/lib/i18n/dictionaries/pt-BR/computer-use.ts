import type { ComputerUseDict } from "../types";

/**
 * Brazilian Portuguese dictionary for `app/[locale]/computer-use/page.tsx`
 * and the Computer Use section of the install page. Product names, the menu
 * items Pause, Stop and Check for updates, and the macOS setting names stay
 * as the app shows them.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use para Mac · Codewhale",
  metaDescription: "Baixe e configure o Codewhale Computer Use para Mac. Controle de apps em segundo plano, configuração de permissões e controles humanos de Pause e Stop.",
  title: "Computer Use",
  lead: "Deixe o Codewhale trabalhar nos seus apps enquanto você continua trabalhando. O assistente para Mac reúne na barra de menus as permissões, o controle de apps em segundo plano e uma forma de pausar ou interromper a entrada.",
  publisher: "Por Codewhale",
  download: "Baixar para Mac",
  downloadZip: "Arquivo ZIP (usado pelo atualizador interno do app)",
  requirements: "macOS 13.5 ou posterior · Apple silicon e Intel",
  included: "Um único download do app. Não é preciso instalar o Node nem um compilador à parte.",
  pendingTitle: "Download para Mac em preparação",
  pendingBody: "O instalador público aparecerá aqui assim que a notarização da Apple e as verificações de lançamento forem concluídas.",
  unavailableTitle: "Não foi possível verificar a disponibilidade do download",
  unavailableBody: "Atualize esta página para tentar de novo ou confira os lançamentos publicados abaixo.",
  releases: "Lançamentos publicados",
  receipt: "Detalhes de verificação do download",
  setup: "Configure seu Mac",
  steps: [
    { title: "Instale o app", body: "Abra a imagem de disco e arraste o Codewhale Computer Use para a pasta Aplicativos. Abra-o a partir de Aplicativos e escolha Computer Use no ícone da baleia na barra de menus." },
    { title: "Revise as permissões", body: "Use os botões de configuração para abrir Acessibilidade e Gravação de Tela nos Ajustes do Sistema. Você decide quais permissões conceder." },
    { title: "Execute a verificação em segundo plano", body: "O assistente abre uma janela de teste descartável, digita um texto e captura essa janela. Depois, verifica se o ponteiro ou o app ativo mudou durante a execução." },
    { title: "Conecte ao Codewhale", body: "Revise, confie e ative o Computer Use no marketplace de plugins do Codewhale. Use o plugin 0.3.1 ou posterior para que as ações locais passem pelos controles Pause e Stop do assistente." },
  ],
  controlsTitle: "Continue trabalhando. Mantenha o controle.",
  controlsBody: "As ações compatíveis operam no app selecionado em segundo plano. Apps e gestos que exigem controle em primeiro plano precisam da sua autorização. O menu mostra o alvo e o modo de entrada; Pause (pausar) suspende a entrada do assistente, e Stop (parar) encerra as sessões existentes do assistente.",
  updateTitle: "Atualizações quando você quiser",
  updateBody: "Escolha Check for updates (buscar atualizações) no app. Antes de instalar uma atualização, ele verifica o download, a assinatura do Codewhale e a notarização da Apple, e mantém a versão anterior do app para recuperação.",
  help: "Configuração e solução de problemas",
  notes: "Notas de lançamento",
  demo: "Veja a verificação em segundo plano",
  source: "Código-fonte e outras plataformas",
  platforms: "Este download é para Mac. No Windows e no Linux, por enquanto, usa-se o plugin a partir do código-fonte com configuração no lado do host.",
  installTitle: "Computer Use para Mac",
  installLead: "Adicione controle de apps em segundo plano com o assistente Computer Use. Configure as permissões do Mac, execute uma verificação em segundo plano e pause ou interrompa a entrada do assistente pela barra de menus.",
  installLink: "Download e configuração do Computer Use",
};
