import { describe, expect, it } from "vitest";
import { readBoundedUrlEncodedForm } from "./bounded-form";

function request(body: string, headers: Record<string, string> = {}): Request {
  return new Request("https://codewhale.net/api/admin/login", {
    method: "POST",
    headers: {
      "content-type": "application/x-www-form-urlencoded",
      ...headers,
    },
    body,
  });
}

describe("readBoundedUrlEncodedForm", () => {
  it("parses the exact login form media type within the byte bound", async () => {
    const form = await readBoundedUrlEncodedForm(request("token=safe%20value&locale=zh"), 64);
    expect(form.get("token")).toBe("safe value");
    expect(form.get("locale")).toBe("zh");
  });

  it("rejects unsupported media types before reading the body", async () => {
    const req = request("token=value", { "content-type": "multipart/form-data; boundary=x" });
    await expect(readBoundedUrlEncodedForm(req, 64)).rejects.toMatchObject({ status: 415 });
  });

  it("rejects oversized declared lengths before reading", async () => {
    const req = request("token=value", { "content-length": "4097" });
    await expect(readBoundedUrlEncodedForm(req, 4096)).rejects.toMatchObject({ status: 413 });
  });

  it("enforces the streaming byte cap when Content-Length is absent", async () => {
    const req = request(`token=${"x".repeat(64)}`);
    req.headers.delete("content-length");
    await expect(readBoundedUrlEncodedForm(req, 16)).rejects.toMatchObject({ status: 413 });
  });

  it("preserves a split multibyte form value exactly at the byte limit", async () => {
    const bytes = new TextEncoder().encode("token=鲸");
    const makeRequest = () => new Request("https://codewhale.net/api/admin/login", {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body: new ReadableStream<Uint8Array>({ start(controller) {
        controller.enqueue(bytes.slice(0, 7));
        controller.enqueue(bytes.slice(7));
        controller.close();
      } }),
      duplex: "half",
    } as RequestInit);
    expect((await readBoundedUrlEncodedForm(makeRequest(), 9)).get("token")).toBe("鲸");
    await expect(readBoundedUrlEncodedForm(makeRequest(), 8)).rejects.toMatchObject({ name: "FormBodyError", status: 413 });
  });

  it("rejects malformed Content-Length values", async () => {
    const req = request("token=value", { "content-length": "not-a-number" });
    await expect(readBoundedUrlEncodedForm(req, 64)).rejects.toMatchObject({ status: 400 });
  });
});
