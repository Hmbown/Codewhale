import type { Metadata } from "next";
import localFont from "next/font/local";
import { IBM_Plex_Mono, Newsreader } from "next/font/google";
import { Nav } from "@/components/nav";
import { Footer } from "@/components/footer";
import { UsageCounting } from "@/components/usage-counting";
import { BUILD_FACTS } from "@/lib/facts";
import { localeDirection, locales, type Locale } from "@/lib/i18n/config";
import { getChrome, getHome } from "@/lib/i18n/dictionaries";
import { serializeJsonLd } from "@/lib/json-ld";
import { buildPageMetadata } from "@/lib/page-meta";
import { buildSiteJsonLd } from "@/lib/site-schema";
import "../globals.css";

// Shannon Sans 0.110 supplies body and small-heading roles through one asset.
// Its OFL notice lives beside it; Newsreader and IBM Plex Mono keep their roles.
const sans = localFont({
  src: "../../public/brand/fonts/ShannonSans-Variable.woff2",
  weight: "100 900",
  style: "normal",
  variable: "--font-shannon-sans",
  display: "swap",
});

// IBM Plex Mono is the GPUI app's code face; it fills the same role here.
const mono = IBM_Plex_Mono({
  subsets: ["latin", "latin-ext", "cyrillic"],
  weight: ["400", "500", "600"],
  variable: "--font-mono",
  display: "swap",
});

// Newsreader's optical-size axis is what lets the same face set a 5rem title
// and a 1.3rem running head without looking like two fonts.
const serif = Newsreader({
  subsets: ["latin", "latin-ext"],
  weight: ["400", "500"],
  style: ["normal", "italic"],
  variable: "--font-serif",
  display: "swap",
});

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }): Promise<Metadata> {
  const { locale } = await params;
  const home = getHome(locale);
  return buildPageMetadata({
    path: "/",
    locale,
    title: home.metaTitle,
    description: home.metaDescription,
  });
}

export default async function LocaleLayout({
  children,
  params,
}: {
  children: React.ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const chrome = getChrome(locale);
  // RTL locales (e.g. ar) set the document direction from the canonical
  // registry so the browser handles bidirectional layout from the root.
  const dir = localeDirection(locale);
  const siteJsonLd = buildSiteJsonLd(locale);

  return (
    <html
      lang={locale}
      dir={dir}
      className={`${sans.variable} ${mono.variable} ${serif.variable}`}
      suppressHydrationWarning
    >
      <body>
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: serializeJsonLd(siteJsonLd) }}
        />
        {/* Apply the persisted docs theme before paint so there is no flash.
            The site default is the paper sheet; only an explicit "dark"
            choice re-themes the docs subtree to the whale's stage. */}
        <script
          dangerouslySetInnerHTML={{
            __html:
              "(function(){try{var t=localStorage.getItem('cw-theme');if(t==='light'||t==='dark'){document.documentElement.setAttribute('data-theme',t);}}catch(e){}})();",
          }}
        />
        <a href="#main-content" className="skip-link">
          {chrome.skipToContent}
        </a>
        <Nav locale={locale as Locale} />
        <main id="main-content">{children}</main>
        <Footer locale={locale as Locale} />
        {/* Aggregate usage counting, on by default — see lib/telemetry. The
            choice lives on the privacy page; every opt-out stays off. */}
        <UsageCounting appVersion={BUILD_FACTS.version ?? "0.0.0"} />
      </body>
    </html>
  );
}
