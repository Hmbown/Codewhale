import { getEnv } from "@/lib/kv";
import { isValidChannel, resolveCloudFacts, responseFor } from "@/lib/cloud-facts";

/**
 * Public, credential-free cloud facts envelope for one channel (facts/v1).
 *
 * GET  /api/facts/v1/<channel>  → signed envelope JSON (strong ETag, CDN-cacheable)
 * HEAD /api/facts/v1/<channel>  → headers only
 *
 * The envelope is verified server-side before it is served; clients verify it
 * again against the keys pinned in the binary. No cookies, no Vary, no query
 * parameters: the response is identical for every caller so any CDN in front
 * (Cloudflare today, Vercel if the host moves) can cache it.
 */
export const dynamic = "force-dynamic";
export const revalidate = 0;

async function handle(req: Request, ctx: { params: Promise<{ channel: string }> }, method: "GET" | "HEAD"): Promise<Response> {
  const { channel } = await ctx.params;
  if (!isValidChannel(channel)) {
    return responseFor({ kind: "none" }, req, channel, method);
  }
  const env = await getEnv();
  const result = await resolveCloudFacts(channel, env);
  return responseFor(result, req, channel, method);
}

export async function GET(req: Request, ctx: { params: Promise<{ channel: string }> }) {
  return handle(req, ctx, "GET");
}

export async function HEAD(req: Request, ctx: { params: Promise<{ channel: string }> }) {
  return handle(req, ctx, "HEAD");
}
