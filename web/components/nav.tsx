import Link from "next/link";
import Image from "next/image";
import type { Locale } from "@/lib/i18n/config";
import { getChrome } from "@/lib/i18n/dictionaries";
import { navLinks, secondaryNavLinks, REPO_URL, APP_LOGIN_URL, APP_SIGNUP_URL } from "@/lib/i18n/links";
import { fetchRepoStats, formatStars } from "@/lib/github";
import { getEnv } from "@/lib/kv";
import { Icon } from "./icon";
import { LocaleSwitcher } from "./locale-switcher";
import { MobileMenu } from "./mobile-menu";
import { NavLinks } from "./nav-links";
import { ThemeToggle } from "./theme-toggle";

/** Masthead + primary nav — the Tideline topbar on the web. */
export async function Nav({ locale = "en" }: { locale?: Locale }) {
  const chrome = getChrome(locale);
  const links = navLinks(locale, chrome);
  const moreLinks = secondaryNavLinks(locale, chrome);
  const homeHref = `/${locale}`;

  // Live star count — cached by fetchRepoStats. Falls back to a plain GitHub
  // label when the API is unreachable at build time.
  let stars = 0;
  try {
    const env = await getEnv();
    stars = (await fetchRepoStats(env.GITHUB_TOKEN)).stars;
  } catch {
    /* keep fallback label */
  }

  return (
    <header className="site-nav paper-nav">
      <div className="site-nav-inner paper-nav-inner">
        <Link href={homeHref} className="site-wordmark paper-wordmark" aria-label={chrome.navHomeAria}>
          <div className="paper-wordmark-text">
            <Image src="/brand/mark-gradient.svg" width={22} height={22} alt="" className="paper-wordmark-mark" unoptimized />
            <img
              className="paper-wordmark-logo"
              src="/brand/wordmark.svg"
              alt=""
              width={142}
              height={20}
            />
          </div>
        </Link>

        <NavLinks links={links} primaryAria={chrome.navPrimaryAria} />

        <div className="site-nav-actions">
          <ThemeToggle
            autoLabel={chrome.themeAuto}
            lightLabel={chrome.themeLight}
            darkLabel={chrome.themeDark}
            ariaTemplate={chrome.themeAria}
            titleLabel={chrome.themeTitle}
          />
          <LocaleSwitcher current={locale} />
          <Link
            href={REPO_URL}
            className="site-github-link paper-star-badge"
            aria-label={chrome.starsAria}
          >
            <Icon name="github" className="brand-mark" />
            ★ {stars > 0 ? formatStars(stars) : chrome.githubFallback}
          </Link>
          <span className="paper-auth" role="group" aria-label={chrome.authGroupAria}>
            <Link href={APP_LOGIN_URL} className="paper-auth-signin hidden lg:inline-flex" data-usage="login">
              {chrome.authSignIn}
            </Link>
            <Link href={APP_SIGNUP_URL} className="paper-auth-register hidden lg:inline-flex" data-usage="signup">
              {chrome.authRegister}
            </Link>
          </span>
          <Link
            href={`/${locale}/install`}
            className="paper-install-cta hidden xl:inline-flex"
          >
            {chrome.installCta}
          </Link>
          <MobileMenu
            installHref={`/${locale}/install`}
            installLabel={chrome.installCta}
            signInHref={APP_LOGIN_URL}
            signInLabel={chrome.authSignIn}
            registerHref={APP_SIGNUP_URL}
            registerLabel={chrome.authRegister}
            links={links}
            moreLinks={moreLinks}
            openLabel={chrome.menuOpen}
            closeLabel={chrome.menuClose}
            navAria={chrome.navPrimaryAria}
          />
        </div>
      </div>
    </header>
  );
}
