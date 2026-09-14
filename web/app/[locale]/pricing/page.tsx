import { redirect } from "next/navigation";

// Public pricing is not offered. Preserve incoming links without advertising
// a dormant plan; use a temporary redirect so future availability is explicit.
export default async function PricingPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  redirect(`/${locale}/install`);
}
