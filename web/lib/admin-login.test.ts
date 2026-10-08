import { beforeEach, describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({
  getAgentEnv: vi.fn(), safeEqual: vi.fn(), createSession: vi.fn(), limit: vi.fn(),
}));
vi.mock("@/lib/community-agent", () => mocks);
import { POST } from "@/app/api/admin/login/route";

function request(token = "wrong", ip = "127.0.0.1") {
  return new Request("https://codewhale.net/api/admin/login?locale=en", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded", "CF-Connecting-IP": ip },
    body: new URLSearchParams({ token }),
  });
}
beforeEach(() => {
  vi.resetAllMocks();
  mocks.getAgentEnv.mockResolvedValue({ MAINTAINER_TOKEN: "configured", ADMIN_LOGIN_LIMITER: { limit: mocks.limit }, CURATED_KV: {} });
  mocks.limit.mockResolvedValue({ success: true });
  mocks.safeEqual.mockResolvedValue(false);
  mocks.createSession.mockResolvedValue("session-id");
});
describe("admin login attempt boundary", () => {
  it("bounds repeated guesses independently of token and forged IP", async () => {
    let attempts = 0;
    mocks.limit.mockImplementation(async () => ({ success: ++attempts <= 5 }));
    for (let index = 0; index < 7; index++) {
      const response = await POST(request(`guess-${index}`, `192.0.2.${index}`));
      expect(response.status).toBe(index < 5 ? 303 : 429);
      if (index >= 5) expect(response.headers.get("Retry-After")).toBe("60");
      expect(response.headers.get("Cache-Control")).toBe("no-store");
    }
    expect(mocks.safeEqual).toHaveBeenCalledTimes(5);
    expect(mocks.createSession).not.toHaveBeenCalled();
    expect(mocks.limit.mock.calls.map(([options]) => options.key)).toEqual(Array(7).fill("codewhale-web:admin-login"));
  });
  it("does not parse or compare credentials after refusal", async () => {
    mocks.limit.mockResolvedValue({ success: false });
    const req = request("configured");
    const read = vi.spyOn(req, "text");
    expect((await POST(req)).status).toBe(429);
    expect(read).not.toHaveBeenCalled();
    expect(mocks.safeEqual).not.toHaveBeenCalled();
    expect(mocks.createSession).not.toHaveBeenCalled();
  });
  it("fails closed if the binding is missing or unavailable", async () => {
    mocks.getAgentEnv.mockResolvedValueOnce({ MAINTAINER_TOKEN: "configured" });
    expect((await POST(request())).status).toBe(503);
    mocks.limit.mockRejectedValue(new Error("binding unavailable"));
    expect((await POST(request())).status).toBe(503);
    expect(mocks.safeEqual).not.toHaveBeenCalled();
    expect(mocks.createSession).not.toHaveBeenCalled();
  });
  it("retains the authenticated session and redirect after an allowed attempt", async () => {
    mocks.safeEqual.mockResolvedValue(true);
    const response = await POST(request("configured"));
    expect(response.status).toBe(303);
    expect(response.headers.get("Location")).toBe("https://codewhale.net/en/admin");
    const cookie = response.headers.get("Set-Cookie");
    expect(cookie).toContain("mt_sid=session-id");
    expect(cookie).toContain("HttpOnly");
    expect(cookie).toContain("Secure");
    expect(cookie?.toLowerCase()).toContain("samesite=strict");
    expect(mocks.createSession).toHaveBeenCalledOnce();
  });
});
