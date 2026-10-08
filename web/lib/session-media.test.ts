import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SessionMedia } from "../components/session-media";
import type { MediaAsset } from "./media-manifest";

const published: MediaAsset = {
  id: "sample",
  title: { en: "Sample", zh: "示例" },
  description: { en: "Sample session", zh: "示例会话" },
  status: "published",
  pendingLabel: { en: "Pending", zh: "待录制" },
  poster: { src: "media/sample.png", width: 1280, height: 720, alt: { en: "Poster", zh: "海报" } },
  video: { src: "media/sample.mp4", durationSeconds: 30, width: 1280, height: 720 },
  captions: [
    { src: "media/sample.en.vtt", srclang: "en", label: "English" },
    { src: "media/sample.zh.vtt", srclang: "zh", label: "中文" },
  ],
};

function defaultTrack(locale: string): string | undefined {
  const html = renderToStaticMarkup(createElement(SessionMedia, { asset: published, locale }));
  const tracks = [...html.matchAll(/<track [^>]*>/g)].map((match) => match[0]);
  const defaults = tracks.filter((track) => /\sdefault(=""|\s|>|\/)/.test(track));
  expect(defaults, html).toHaveLength(1);
  return defaults[0].match(/srcLang="([^"]+)"/i)?.[1];
}

describe("SessionMedia captions", () => {
  it("defaults to the page's caption track, else English (W01-m2)", () => {
    expect(defaultTrack("zh")).toBe("zh");
    expect(defaultTrack("en")).toBe("en");
    // A routed locale with no caption track of its own falls back to English.
    expect(defaultTrack("ja")).toBe("en");
  });
});
