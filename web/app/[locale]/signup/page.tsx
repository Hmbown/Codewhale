import { ACCOUNT_ENTRY_COPY } from "@/lib/content/account-entry";
import { pickText } from "@/lib/i18n/dictionaries";
import { PublicAccountEntry } from "@/components/public-account-entry";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({
    path: "/signup",
    locale,
    title: pickText(ACCOUNT_ENTRY_COPY.signUp.metaTitle, locale),
    description: pickText(ACCOUNT_ENTRY_COPY.metaDescription, locale),
  });
}

export default async function SignUpPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <PublicAccountEntry locale={locale} kind="sign-up" />;
}
