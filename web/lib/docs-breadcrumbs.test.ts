import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  buildBreadcrumbListJsonLd,
  resolveDocsBreadcrumbs,
  resolveDocsTopic,
} from "./docs-breadcrumbs";
import { SITE_URL } from "./page-meta";

describe("docs breadcrumbs", () => {
  it("stops the hub trail at Docs", () => {
    expect(resolveDocsBreadcrumbs("en", "/en/docs")).toEqual([
      { name: "Home", href: "/en" },
      { name: "Docs" },
    ]);
    expect(resolveDocsBreadcrumbs("zh", "/zh/docs/")).toEqual([
      { name: "首页", href: "/zh" },
      { name: "文档" },
    ]);
  });

  it("names the category and topic on a first-party docs page", () => {
    expect(resolveDocsBreadcrumbs("en", "/en/docs/mcp")).toEqual([
      { name: "Home", href: "/en" },
      { name: "Docs", href: "/en/docs" },
      { name: "Extend and automate" },
      { name: "Connect tools with MCP" },
    ]);
    expect(resolveDocsBreadcrumbs("zh", "/zh/docs/modes/")).toEqual([
      { name: "首页", href: "/zh" },
      { name: "文档", href: "/zh/docs" },
      { name: "日常使用" },
      { name: "设置模式与审批" },
    ]);
    expect(resolveDocsTopic("en", "/en/docs/guide")?.id).toBe("guide");
  });

  it("emits BreadcrumbList with absolute Home → Docs → topic URLs", () => {
    const schema = buildBreadcrumbListJsonLd("en", "/en/docs/sandbox");
    expect(schema["@type"]).toBe("BreadcrumbList");
    expect(schema.itemListElement).toEqual([
      {
        "@type": "ListItem",
        position: 1,
        name: "Home",
        item: `${SITE_URL}/en`,
      },
      {
        "@type": "ListItem",
        position: 2,
        name: "Docs",
        item: `${SITE_URL}/en/docs`,
      },
      {
        "@type": "ListItem",
        position: 3,
        name: "Limit what commands can touch",
        item: `${SITE_URL}/en/docs/sandbox`,
      },
    ]);
  });

  it("renders the trail from the docs shell", () => {
    const layout = readFileSync(new URL("../app/[locale]/docs/layout.tsx", import.meta.url), "utf8");
    expect(layout).toContain("<DocsBreadcrumb locale={locale} />");
    expect(layout).toContain('import { DocsBreadcrumb } from "@/components/docs-breadcrumb"');
  });
});
