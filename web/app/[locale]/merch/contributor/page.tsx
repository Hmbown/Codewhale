import type { Metadata } from "next";
import { ContributorMerch } from "@/components/contributor-merch";
import { getMerchCopy } from "@/lib/content/contributor-merch";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }): Promise<Metadata> {
  const { locale } = await params;
  const copy = getMerchCopy(locale);
  return { title: `${copy.title} · Codewhale`, description: copy.lede, robots: { index: false, follow: false } };
}

export default async function ContributorMerchPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <ContributorMerch locale={locale} />;
}
