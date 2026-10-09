import type { Metadata } from "next";
import { MerchStorefront } from "@/components/merch-storefront";
import { getStorefrontCopy } from "@/lib/content/merch-storefront";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }): Promise<Metadata> {
  const { locale } = await params;
  const copy = getStorefrontCopy(locale);
  return { title: `${copy.merch} · Codewhale`, description: copy.description, robots: { index: false, follow: false } };
}

export default async function MerchPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <MerchStorefront locale={locale} />;
}
