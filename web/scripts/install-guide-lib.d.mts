export type InstallGuide = {
  sourceHash: string;
  anchors: string[];
  chunks: { kind: "html" | "code"; text: string }[];
};
export function buildInstallGuide(source: string): InstallGuide;
export function installAnchorErrors(text: string, anchors: readonly string[]): string[];
export function renderInstallGuideModule(source: string): string;
