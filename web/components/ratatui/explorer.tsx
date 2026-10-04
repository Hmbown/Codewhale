"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { Icon } from "@/components/icon";
import type { RatatuiCopy } from "@/lib/content/ratatui";
import { pickText } from "@/lib/i18n/dictionaries";
import { motionId, previewAssetPath, searchEntries, type Catalogue, type EntryPreview } from "@/lib/ratatui/catalogue";
import { TASKS, displayApi, filterByTask, getGuidance } from "@/lib/ratatui/learning";
import { CodeBlock } from "./highlight";
import { getRecipe, recipeSourceUrl, learningGuideUrl } from "@/lib/ratatui/recipes";
import "./explorer.css";

type MotionTrack = {
  id: string;
  title: string;
  profile: string;
  width: number;
  height: number;
  frameMs: number;
  frames: string[];
  source: string;
};

export function CopyButton({ text, copy }: { text: string; copy: RatatuiCopy }) {
  const [status, setStatus] = useState<"idle" | "copied" | "error">("idle");
  useEffect(() => {
    if (status === "idle") return;
    const timer = window.setTimeout(() => setStatus("idle"), 2400);
    return () => window.clearTimeout(timer);
  }, [status]);
  return (
    <span className="rat-copy">
      <button className="rat-button" type="button" disabled={!text} onClick={async () => {
        try { await navigator.clipboard.writeText(text); setStatus("copied"); }
        catch { setStatus("error"); }
      }}><Icon name={status === "copied" ? "check" : "copy"} />{status === "copied" ? copy.copied : copy.copy}</button>
      <span className="rat-copy-status" role="status">{status === "error" ? copy.copyFailed : ""}</span>
    </span>
  );
}

/** SVGs are actual library buffers, displayed as isolated images rather than injected markup. */
function TerminalImage({ svg, label, fit, width, height }: { svg: string; label: string; fit: boolean; width: number; height: number }) {
  const [url, setUrl] = useState("");
  useEffect(() => {
    const objectUrl = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
    setUrl(objectUrl);
    return () => URL.revokeObjectURL(objectUrl);
  }, [svg]);
  // Small buffers would sit tiny in a wide panel; vector SVGs scale without blur.
  // Stylesheet width beats the sizing attributes, so scaled renders set it.
  const scale = fit && width <= 60 ? 2 : 1;
  return <div className={`rat-terminal${fit ? " rat-terminal-fit" : ""}${scale > 1 ? " rat-terminal-scaled" : ""}`} role="region" aria-label={label} tabIndex={0} dir="ltr">
    {/* next/image cannot optimize browser-owned blob URLs; the SVG retains its precise cell grid. */}
    {/* eslint-disable-next-line @next/next/no-img-element */}
    {url && <img src={url} alt={label} width={width * 10 * scale} height={height * 20 * scale} style={scale > 1 ? { width: width * 10 * scale, height: "auto" } : undefined} />}
  </div>;
}

function MotionPlayer({ id, copy, fit }: { id: string; copy: RatatuiCopy; fit: boolean }) {
  const [track, setTrack] = useState<MotionTrack | null>(null);
  const [error, setError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [frame, setFrame] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => { setReduced(media.matches); if (media.matches) setPlaying(false); };
    update(); media.addEventListener("change", update);
    const hidden = () => { if (document.hidden) setPlaying(false); };
    document.addEventListener("visibilitychange", hidden);
    return () => { media.removeEventListener("change", update); document.removeEventListener("visibilitychange", hidden); };
  }, []);
  useEffect(() => {
    const controller = new AbortController();
    setTrack(null); setError(false); setPlaying(false); setFrame(0);
    fetch(`/ratatui/motion/${id}.json`, { signal: controller.signal })
      .then((response) => { if (!response.ok) throw new Error("Motion unavailable"); return response.json(); })
      .then((data: MotionTrack) => {
        if (data.id !== id || !Array.isArray(data.frames) || data.frames.length < 2 || !data.frameMs) throw new Error("Invalid motion track");
        setTrack(data);
      }).catch(() => { if (!controller.signal.aborted) setError(true); });
    return () => controller.abort();
  }, [id, attempt]);
  useEffect(() => {
    if (!playing || !track) return;
    const timer = window.setInterval(() => setFrame((value) => (value + 1) % track.frames.length), track.frameMs / speed);
    return () => window.clearInterval(timer);
  }, [playing, track, speed]);
  if (error) return <div className="rat-load" role="alert"><p>{copy.previewError}</p><button className="rat-button" onClick={() => setAttempt((value) => value + 1)}>{copy.retry}</button></div>;
  if (!track) return <div className="rat-load" role="status">{copy.loading}</div>;
  return <div className="rat-motion">
    <div className="rat-motion-intro"><p>{copy.relatedMotion}: {track.title}</p><span>{track.profile} · {copy.frameCount.replace("{count}", String(track.frames.length))}</span></div>
    <TerminalImage svg={track.frames[frame]} label={`${track.title} · ${copy.frame} ${frame + 1}`} fit={fit} width={track.width} height={track.height} />
    <div className="rat-motion-controls">
      <button type="button" className="rat-button rat-button-primary" onClick={() => setPlaying(!playing)} aria-pressed={playing}>{playing ? copy.pause : copy.play}</button>
      <button type="button" className="rat-button" onClick={() => { setFrame(0); setPlaying(false); }}>{copy.reset}</button>
      <label className="rat-scrubber">{copy.frame}<input aria-label={copy.frame} type="range" min="0" max={track.frames.length - 1} value={frame} onChange={(event) => { setPlaying(false); setFrame(Number(event.target.value)); }} /><output>{frame + 1}/{track.frames.length}</output></label>
      <label className="rat-field">{copy.speed}<select aria-label={copy.speed} value={speed} onChange={(event) => setSpeed(Number(event.target.value))}><option value="0.5">0.5×</option><option value="1">1×</option><option value="2">2×</option></select></label>
    </div>
    <p className="rat-note">{reduced ? copy.reducedMotionNote : copy.motionDescription} <a href={track.source}>{copy.openSource}<Icon name="external" /></a></p>
  </div>;
}

/** This explorer switches captured library output; the interactive native host is the Cargo example. */
export function RatatuiExplorer({ catalogue, locale, copy, initialEntry, detail }: { catalogue: Catalogue; locale: string; copy: RatatuiCopy; initialEntry?: string; detail?: boolean }) {
  const first = catalogue.entries.find((entry) => entry.name === (initialEntry ?? "showcase-work")) ?? catalogue.entries[0];
  const router = useRouter();
  const [selected, setSelected] = useState(first.name);
  const [query, setQuery] = useState("");
  const [family, setFamily] = useState("all");
  const [task, setTask] = useState("all");
  const [browserExpanded, setBrowserExpanded] = useState(true);
  const [mobile, setMobile] = useState(false);
  const [profile, setProfile] = useState("dark-truecolor");
  const [size, setSize] = useState("native");
  const [fit, setFit] = useState(true);
  const [tab, setTab] = useState<"preview" | "use" | "code" | "motion">("preview");
  useEffect(() => {
    const requested = new URLSearchParams(window.location.search).get("tab");
    if (requested === "use" || requested === "code" || requested === "motion") setTab(requested);
  }, []);
  useEffect(() => {
    const url = new URL(window.location.href);
    if (tab === "preview") url.searchParams.delete("tab");
    else url.searchParams.set("tab", tab);
    window.history.replaceState(null, "", url);
  }, [tab]);
  const [data, setData] = useState<EntryPreview | null>(null);
  const [error, setError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);
  const familyEntries = useMemo(() => catalogue.entries.filter((item) => item.family === first.family), [catalogue.entries, first.family]);
  const filtered = useMemo(() => detail
    ? familyEntries
    : searchEntries(filterByTask(catalogue.entries, task), query, family),
    [catalogue.entries, query, family, task, detail, familyEntries]);
  const entry = filtered.find((item) => item.name === selected) ?? filtered[0] ?? first;
  const collection = catalogue.families.find((item) => item.id === entry.family);
  const guidance = getGuidance(entry, locale);
  const recipe = getRecipe(guidance.recipeId);
  const animation = motionId(entry, profile);
  const activeTab = tab === "motion" && !animation ? "preview" : tab;
  const currentIndex = filtered.findIndex((item) => item.name === entry.name);
  const svg = data?.previews[profile]?.[size];
  const label = copy.terminalPreviewLabel.replace("{name}", entry.title);
  const sourceUrl = entry.source.url;
  const columns = size === "native" ? entry.width : Number(size);
  useEffect(() => {
    const controller = new AbortController();
    setData(null); setError(false);
    fetch(previewAssetPath(entry), { signal: controller.signal })
      .then((response) => { if (!response.ok) throw new Error("Preview unavailable"); return response.json(); })
      .then((preview: EntryPreview) => {
        if (preview.name !== entry.name || !preview.previews) throw new Error("Invalid preview");
        setData(preview);
      }).catch(() => { if (!controller.signal.aborted) setError(true); });
    return () => controller.abort();
  }, [entry, attempt]);
  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (event.key === "/" && !event.metaKey && !event.ctrlKey && !event.altKey && !target.closest("input,textarea,select,[contenteditable]")) {
        event.preventDefault(); searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);
  useEffect(() => {
    const media = window.matchMedia("(max-width: 760px)");
    const update = () => { setMobile(media.matches); setBrowserExpanded(!media.matches); if (media.matches) setSize("40"); };
    update(); media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  const choose = (name: string) => {
    if (detail) {
      const params = tab === "preview" ? "" : `?tab=${tab}`;
      router.push(`/${locale}/ratatui/${name}${params}`);
      return;
    }
    setSelected(name); setTab("preview"); if (mobile) setBrowserExpanded(false);
  };
  const helpers = data?.fixture.helpers?.map((helper) => helper.code).join("\n\n") ?? "";
  const example = data ? [data.fixture.imports, data.fixture.code, helpers].filter(Boolean).join("\n\n") : "";
  const clearFilters = () => { setQuery(""); setFamily("all"); setTask("all"); };
  return <section id="ratatui-explorer" className={`rat-explorer${detail ? " rat-detail" : ""}`} aria-label={copy.title}>
    {!detail && <div className="rat-task-picker">
      <div><h2>{copy.tasksTitle}</h2><p>{copy.tasksDescription}</p></div>
      <div className="rat-task-options" role="group" aria-label={copy.tasksTitle}>
        <button type="button" className="rat-button" aria-pressed={task === "all"} onClick={clearFilters}>{copy.allComponents}</button>
        {TASKS.map((item) => <button key={item.id} type="button" className="rat-button" aria-pressed={task === item.id} onClick={() => {
          setTask(item.id); setFamily("all"); setQuery("");
          choose(item.recommended.find((name) => catalogue.entries.some((candidate) => candidate.name === name)) ?? first.name);
        }}>{pickText(item.label, locale)}</button>)}
      </div>
      {task !== "all" && <p className="rat-task-description">{pickText(TASKS.find((item) => item.id === task)!.description, locale)}</p>}
    </div>}
    {detail ? <aside className="rat-browser rat-family-index" aria-label={collection?.title ?? copy.componentNavLabel}>
      <h2>{collection?.title}</h2>
      <p className="rat-count" role="status">{copy.familyPosition.replace("{index}", String(currentIndex + 1)).replace("{count}", String(filtered.length))}</p>
      <nav className="rat-entry-list" aria-label={collection?.title}>
        {filtered.map((candidate) => <Link key={candidate.name} href={`/${locale}/ratatui/${candidate.name}`} className="rat-entry" aria-current={candidate.name === entry.name ? "page" : undefined}><span>{candidate.title}</span><span className="rat-entry-width">{candidate.width}</span></Link>)}
      </nav>
      <Link href={`/${locale}/ratatui`} className="rat-link rat-index-back">{copy.back}<Icon name="arrow-right" className="rat-prev-icon" /></Link>
    </aside> : <aside className="rat-browser" aria-label={copy.componentNavLabel}>
      <label className="rat-search" htmlFor="rat-search"><span>{copy.searchLabel}</span><div><Icon name="search" /><input id="rat-search" ref={searchRef} type="search" value={query} placeholder={copy.searchPlaceholder} onChange={(event) => { setQuery(event.target.value); if (mobile) setBrowserExpanded(true); }} onKeyDown={(event) => { if (event.key === "Escape") setQuery(""); }} /><kbd>/</kbd></div></label>
      <label className="rat-family-field" htmlFor="rat-family">{copy.families}<select id="rat-family" value={family} onChange={(event) => { setFamily(event.target.value); setTask("all"); if (mobile) setBrowserExpanded(true); }}><option value="all">{copy.allComponents}</option>{catalogue.families.map((item) => <option key={item.id} value={item.id}>{item.title} ({item.count})</option>)}</select></label>
      <p className="rat-count" role="status">{copy.visibleCount.replace("{visible}", String(filtered.length)).replace("{total}", String(catalogue.entries.length))}</p>
      <details className="rat-browser-results" open={browserExpanded} onToggle={(event) => setBrowserExpanded(event.currentTarget.open)}><summary>{copy.allComponents}<Icon name="chevron-down" /></summary>
      <nav className="rat-entry-list" aria-label={copy.resultsLabel}>
        {catalogue.families.map((item) => {
          const entries = filtered.filter((candidate) => candidate.family === item.id);
          return entries.length ? <div className="rat-family" key={item.id}><h3>{item.title} <span className="rat-family-count">{entries.length}</span></h3>{entries.map((candidate) => <button type="button" key={candidate.name} className="rat-entry" aria-current={candidate.name === entry.name ? "true" : undefined} onClick={() => choose(candidate.name)}><span>{candidate.title}</span><span className="rat-entry-width">{candidate.width}</span></button>)}</div> : null;
        })}
      </nav></details>
    </aside>}
    {filtered.length === 0 ? <div className="rat-study rat-empty-study"><h2>{copy.emptyTitle}</h2><p>{copy.emptyDescription}</p><button className="rat-button" onClick={clearFilters}>{copy.clearSearch}</button></div> : <div className="rat-study">
      {!detail && <><div className="rat-study-heading"><div><p>{collection?.title}</p><h2>{entry.title}</h2></div>{initialEntry !== entry.name && <Link href={`/${locale}/ratatui/${entry.name}`} className="rat-link" title={copy.share}>{copy.share}<Icon name="arrow-right" /></Link>}</div>
      <p className="rat-description">{entry.description}</p></>}
      <div className="rat-study-tabs" role="group" aria-label={copy.preview}>
        <button type="button" aria-pressed={activeTab === "preview"} onClick={() => setTab("preview")}>{copy.preview}</button>
        <button type="button" aria-pressed={activeTab === "use"} onClick={() => setTab("use")}>{copy.use}</button>
        <button type="button" aria-pressed={activeTab === "code"} onClick={() => setTab("code")}>{copy.code}</button>
        {animation && <button type="button" aria-pressed={activeTab === "motion"} onClick={() => { setTab("motion"); if (profile !== "light-truecolor") setProfile("dark-truecolor"); }}>{copy.motion}</button>}
        <div className="rat-pager"><button type="button" aria-label={copy.previous} disabled={currentIndex <= 0} onClick={() => choose(filtered[currentIndex - 1].name)}><Icon name="arrow-right" className="rat-prev-icon" /></button><button type="button" aria-label={copy.next} disabled={currentIndex < 0 || currentIndex >= filtered.length - 1} onClick={() => choose(filtered[currentIndex + 1].name)}><Icon name="arrow-right" /></button></div>
      </div>
      {(activeTab === "preview" || activeTab === "motion") && <div className="rat-preview-controls">{(activeTab !== "motion" || !["whale", "habitat"].includes(animation ?? "")) && <label className="rat-field">{copy.profile}<select aria-label={copy.profile} value={profile} onChange={(event) => setProfile(event.target.value)}>{catalogue.profiles.filter((item) => activeTab !== "motion" || ["dark-truecolor", "light-truecolor"].includes(item.id)).map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>}{activeTab === "preview" && <label className="rat-field">{copy.size}<select aria-label={copy.size} value={size} onChange={(event) => setSize(event.target.value)}>{catalogue.sizes.map((item) => <option key={item.id} value={item.id}>{item.id === "native" ? copy.nativeSize : copy.columns.replace("{count}", item.id)}</option>)}</select></label>}<button type="button" className="rat-button rat-scale" onClick={() => setFit(!fit)} aria-pressed={!fit}>{fit ? copy.actualSize : copy.fit}</button></div>}
      {activeTab === "use" ? <div className="rat-code-panel rat-usage-panel">
        <p>{guidance.description}</p><p className="rat-note">{guidance.hostNote}</p>
        {recipe && <><div className="rat-code-heading"><h4>{copy.use}</h4><CopyButton text={recipe.code} copy={copy} /></div><CodeBlock code={recipe.code} language="rust" /><p className="rat-note">{copy.recipeNote} <a href={recipeSourceUrl(recipe.line)}>{copy.openSource}<Icon name="external" /></a></p></>}
        <div className="rat-usage-links"><a className="rat-link" href={learningGuideUrl(guidance.guideAnchor)}>{copy.guide}<Icon name="external" /></a><Link className="rat-link" href={`/${locale}/ratatui#ratatui-install`}>{copy.gettingStarted}<Icon name="arrow-right" /></Link></div>
        <p className="rat-note">{copy.runExample}</p><CodeBlock code={`cargo run --locked --example ${recipe ? "recipes" : guidance.example.replace(/^examples\//, "").replace(/\.rs$/, "")}`} language="sh" />{recipe && <p className="rat-note">{copy.recipeControls}</p>}
      </div> :
      activeTab === "motion" && animation ? <MotionPlayer key={animation} id={animation} copy={copy} fit={fit} /> : activeTab === "code" ? <div className="rat-code-panel"><div className="rat-code-heading"><h3>{copy.fixtureTitle}</h3><CopyButton text={example} copy={copy} /></div>{data ? <CodeBlock code={example} language="rust" /> : <div className="rat-load" role={error ? "alert" : "status"}><p>{error ? copy.previewError : copy.loading}</p>{error && <button className="rat-button" onClick={() => setAttempt((value) => value + 1)}>{copy.retry}</button>}</div>}<p className="rat-note">{copy.fixtureNote} <a href={sourceUrl}>{copy.openSource}<Icon name="external" /></a></p></div> : <>
        {error ? <div className="rat-load" role="alert"><p>{copy.previewError}</p><button className="rat-button" type="button" onClick={() => setAttempt((value) => value + 1)}>{copy.retry}</button></div> : svg ? <TerminalImage svg={svg} label={label} fit={fit} width={columns} height={entry.height} /> : <div className="rat-load" role="status">{copy.previewLoading}</div>}
        <div className="rat-preview-meta"><span>{columns} × {entry.height}</span><span>{profile}</span>{svg && <a className="rat-link" href={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`} download={`${entry.name}.${profile}.${size}.svg`}>{copy.downloadSvg}<Icon name="external" /></a>}</div>
        <p className="rat-note">{copy.renderedPreviewNote}</p>
      </>}
      <div className="rat-api"><h3>{copy.api}</h3><div>{displayApi(entry).map((symbol) => <a key={symbol} href={sourceUrl}><code>{symbol}</code></a>)}</div><p>{copy.hostNote}</p></div>
    </div>}
  </section>;
}
