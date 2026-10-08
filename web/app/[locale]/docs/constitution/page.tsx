import { permanentRedirect } from "next/navigation";

/** `/constitution` is the canonical constitution page; this docs URL only redirects there. */
export default async function DocsConstitutionRedirect({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  permanentRedirect(`/${locale}/constitution`);
}
