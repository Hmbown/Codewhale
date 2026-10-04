import { Icon } from "@/components/icon";
import type { RatatuiCopy } from "@/lib/content/ratatui";
import "./project-links.css";

const REPOSITORY = "https://github.com/Hmbown/codewhale-ratatui";

/** Library provenance, rendered once per page instead of once per selection. */
export function RatatuiProjectLinks({ revision, copy }: { revision: string; copy: RatatuiCopy }) {
  return (
    <aside className="rat-project" aria-label={copy.projectStatus}>
      <h2>{copy.projectStatus}</h2>
      <div className="rat-project-links">
        <a href={`${REPOSITORY}/commit/${revision}`}>{copy.sourceRevision}<code>{revision.slice(0, 7)}</code></a>
        <a href={`${REPOSITORY}/actions/workflows/ci.yml?query=branch%3Amain`}>{copy.buildChecks}<Icon name="external" /></a>
        <a href={`${REPOSITORY}/actions/workflows/gallery.yml?query=branch%3Amain`}>{copy.galleryChecks}<Icon name="external" /></a>
        <a href={`${REPOSITORY}/releases`}>{copy.releaseHistory}<Icon name="external" /></a>
      </div>
    </aside>
  );
}
