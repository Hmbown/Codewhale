import Link from "next/link";
import Image from "next/image";
import { InstallCodeBlock } from "@/components/install-code-block";
import { InstallBinary } from "@/components/install-binary";
import { GETTING_STARTED_STEPS } from "@/lib/content/getting-started";
import { INSTALL_COPY } from "@/lib/content/install";
import { COMPUTER_USE_COPY } from "@/lib/content/computer-use";
import type { LocalizedText } from "@/lib/content/vocabulary";
import { fill, getHome, pickText } from "@/lib/i18n/dictionaries";
import { getFacts } from "@/lib/facts";
import { buildPageMetadata } from "@/lib/page-meta";

export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({
    path: "/install",
    locale,
    title: pickText(INSTALL_COPY.metaTitle, locale),
    description: pickText(INSTALL_COPY.metaDescription, locale),
  });
}

const SHELL_INSTALL = `curl -fsSL https://codewhale.net/install.sh | sh`;
const NPM_INSTALL = `npm install -g codewhale`;
const CARGO_INSTALL = `cargo install codewhale-cli --locked`;
const UPDATE = `codewhale update`;

const cnbInstall = (tag: string) =>
  `cargo install --git https://cnb.cool/codewhale.net/codewhale --tag ${tag} codewhale-cli --locked --force`;
const TUNA_CONFIG = `# ~/.cargo/config.toml
[source.crates-io]
replace-with = "tuna"

[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"`;
const TUNA_INSTALL = `cargo install codewhale-cli --locked`;

const BREW = `brew tap Hmbown/deepseek-tui
brew install codewhale`;

const DOCKER = `docker volume create codewhale-home
docker run --rm -it \\
  -e DEEPSEEK_API_KEY=$DEEPSEEK_API_KEY \\
  -v codewhale-home:/home/codewhale/.codewhale \\
  -v "$PWD:/workspace" -w /workspace \\
  ghcr.io/hmbown/codewhale:latest`;

const FROM_SOURCE = `git clone https://github.com/Hmbown/CodeWhale
cd CodeWhale
cargo build --release --locked

# Install the compiled runtime as codewhale
cargo install --path crates/cli --locked`;


export default async function InstallPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const facts = await getFacts();
  const publishedRelease = facts.latestPublishedRelease;
  const t = (copy: LocalizedText) => pickText(copy, locale);
  const home = getHome(locale);
  const copyProps = { copyLabel: home.copy, copiedLabel: home.copied };
  const firstSteps = GETTING_STARTED_STEPS.filter((step) =>
    step.id === "connect-provider" || step.id === "first-session");
  const alternatives = [
    { title: "npm · Node 18+", body: INSTALL_COPY.npmLead, command: NPM_INSTALL },
    { title: t(INSTALL_COPY.cargo), body: INSTALL_COPY.cargoLead, command: CARGO_INSTALL },
    { title: "Homebrew · macOS / Linux", body: INSTALL_COPY.brewLead, command: BREW },
    { title: "Docker", body: INSTALL_COPY.dockerLead, command: DOCKER },
    { title: t(INSTALL_COPY.source), body: INSTALL_COPY.sourceLead, command: FROM_SOURCE },
  ];

  return (
    <div className="install-page">
      <section className="hero">
        <div className="portal-container community-welcome-inner">
          <h1>{t(INSTALL_COPY.title)}</h1>
          <p>{t(INSTALL_COPY.lead)}</p>
          <div className="mt-6 max-w-3xl"><InstallCodeBlock cmd={SHELL_INSTALL} {...copyProps} /></div>
          <p className="text-sm">{t(INSTALL_COPY.installer)}</p>
          <div className="portal-actions">
            <Link href={`/${locale}/computer-use`} className="body-link">{t(COMPUTER_USE_COPY.installLink)}</Link>
            <a href="#other-ways" className="body-link">{t(INSTALL_COPY.other)}</a>
            <a href="/install.sh" className="body-link">{t(INSTALL_COPY.inspect)}</a>
          </div>
        </div>
      </section>

      <section id="computer-use" className="portal-section portal-section-muted scroll-mt-24">
        <div className="portal-container">
          <div className="flex items-center gap-4 mb-4">
            <Image src="/brand/computer-use.png" width={48} height={48} alt="" className="shrink-0" />
            <h2>{t(COMPUTER_USE_COPY.installTitle)}</h2>
          </div>
          <p className="max-w-3xl text-ink-soft leading-relaxed">{t(COMPUTER_USE_COPY.installLead)}</p>
          <Link href={`/${locale}/computer-use`} className="portal-button portal-button-primary mt-5">{t(COMPUTER_USE_COPY.installLink)}</Link>
        </div>
      </section>

      <section className="portal-section">
        <div className="portal-container">
          <h2 className="mb-6">{t(INSTALL_COPY.firstRun)}</h2>
          <ol className="grid md:grid-cols-2 gap-8">
            {firstSteps.map((step, index) => (
              <li key={step.id}>
                <h3 className="mb-3">{index + 1}. {t(step.title)}</h3>
                <p className="mb-4 text-ink-soft leading-relaxed">{t(step.body)}</p>
                <InstallCodeBlock cmd={step.commands.join("\n")} {...copyProps} />
                <Link href={`/${locale}${step.link.href}`} className="body-link mt-3 inline-block">{t(step.link.label)}</Link>
              </li>
            ))}
          </ol>
          <p className="mt-8 max-w-3xl text-ink-soft leading-relaxed">{t(INSTALL_COPY.modes)}</p>
          <Link href={`/${locale}/docs/guide`} className="body-link mt-4 inline-block">{t(INSTALL_COPY.guide)}</Link>
        </div>
      </section>

      <section className="portal-section portal-section-muted">
        <div className="portal-container grid md:grid-cols-2 gap-8">
          <div>
            <h2>{t(INSTALL_COPY.verify)}</h2>
            <p className="my-4 text-ink-soft leading-relaxed">{t(INSTALL_COPY.verifyLead)}</p>
            <InstallCodeBlock cmd={"codewhale --version\ncodewhale doctor"} {...copyProps} />
            <a href={publishedRelease?.url ?? "https://github.com/Hmbown/CodeWhale/releases/latest"} className="body-link mt-3 inline-block">
              {publishedRelease ? fill(t(INSTALL_COPY.latest), { tag: publishedRelease.tag }) : t(INSTALL_COPY.latestUnavailable)}
            </a>
          </div>
          <div>
            <h2>{t(INSTALL_COPY.update)}</h2>
            <p className="my-4 text-ink-soft leading-relaxed">{t(INSTALL_COPY.updateLead)}</p>
            <InstallCodeBlock cmd={UPDATE} {...copyProps} />
            <div className="mt-3"><InstallCodeBlock cmd={"npm update -g codewhale\n# or\nbrew upgrade codewhale"} {...copyProps} /></div>
          </div>
        </div>
      </section>

      <section id="other-ways" className="portal-section scroll-mt-24">
        <div className="portal-container">
          <h2>{t(INSTALL_COPY.alternatives)}</h2>
          <p className="my-4 text-ink-soft leading-relaxed">{t(INSTALL_COPY.alternativesLead)}</p>
          <div className="py-6 hairline-t hairline-b">
            <h3>{t(INSTALL_COPY.binaries)}</h3>
            <p className="my-4 text-ink-soft leading-relaxed">{t(INSTALL_COPY.binariesLead)}</p>
            <InstallBinary {...copyProps} verifyHeading={t(INSTALL_COPY.checksum)} />
          </div>
          {alternatives.map((option) => (
            <div className="py-6 hairline-b" key={option.title}>
              <h3>{option.title}</h3>
              <p className="my-4 text-ink-soft leading-relaxed max-w-3xl">{t(option.body)}</p>
              <InstallCodeBlock cmd={option.command} {...copyProps} />
            </div>
          ))}
        </div>
      </section>

      <section className="portal-section portal-section-muted">
        <div className="portal-container">
          <h2 className="mb-6">{t(INSTALL_COPY.mirrors)}</h2>
          <h3>{t(INSTALL_COPY.cnb)}</h3>
          <p className="my-4 max-w-3xl text-ink-soft leading-relaxed">{t(INSTALL_COPY.cnbLead)}</p>
          {publishedRelease && <InstallCodeBlock cmd={cnbInstall(publishedRelease.tag)} {...copyProps} />}
          <h3 className="mt-8">TUNA</h3>
          <p className="my-4 max-w-3xl text-ink-soft leading-relaxed">{t(INSTALL_COPY.tunaLead)}</p>
          <InstallCodeBlock cmd={TUNA_CONFIG} {...copyProps} />
          <div className="mt-3"><InstallCodeBlock cmd={TUNA_INSTALL} {...copyProps} /></div>
          <a href="https://github.com/Hmbown/CodeWhale/blob/main/docs/CNB_MIRROR.md" className="body-link mt-4 inline-block">{t(INSTALL_COPY.mirrorDocs)}</a>
        </div>
      </section>

      <section className="portal-section">
        <div className="portal-container">
          <h2 className="mb-6">{t(INSTALL_COPY.next)}</h2>
          <div className="portal-actions">
            <Link href={`/${locale}/docs/configuration`} className="body-link">{t(INSTALL_COPY.config)}</Link>
            <Link href={`/${locale}/models`} className="body-link">{t(INSTALL_COPY.models)}</Link>
            <Link href={`/${locale}/faq`} className="body-link">{t(INSTALL_COPY.help)}</Link>
          </div>
        </div>
      </section>
    </div>
  );
}
