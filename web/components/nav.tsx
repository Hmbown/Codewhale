import Link from "next/link";
import Image from "next/image";
import type { Locale } from "@/lib/i18n/config";
import { getChrome } from "@/lib/i18n/dictionaries";
import { navLinks, secondaryNavLinks, REPO_URL, APP_LOGIN_URL } from "@/lib/i18n/links";
import { Icon } from "./icon";
import { LocaleSwitcher } from "./locale-switcher";
import { MobileMenu } from "./mobile-menu";
import { NavLinks } from "./nav-links";
import { ThemeToggle } from "./theme-toggle";

/**
 * Masthead + primary nav. Like the GPUI titlebar it carries few visible
 * controls: the mark, four links, quiet icon controls with accessible names,
 * one identity door (Sign in; the sign-in page offers account creation) and
 * the one primary action, Install.
 */
export function Nav({ locale = "en" }: { locale?: Locale }) {
  const chrome = getChrome(locale);
  const links = navLinks(locale, chrome);
  const moreLinks = secondaryNavLinks(locale, chrome);
  const homeHref = `/${locale}`;

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
          <Link href={REPO_URL} className="nav-icon-button site-github-link" aria-label="GitHub">
            <Icon name="github" className="nav-icon" />
          </Link>
          <Link href={APP_LOGIN_URL} className="paper-auth-signin hidden lg:inline-flex" data-usage="login">
            {chrome.authSignIn}
          </Link>
          <Link
            href={`/${locale}/install`}
            className="paper-install-cta hidden lg:inline-flex"
          >
            {chrome.installCta}
          </Link>
          <MobileMenu
            installHref={`/${locale}/install`}
            installLabel={chrome.installCta}
            signInHref={APP_LOGIN_URL}
            signInLabel={chrome.authSignIn}
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
