import { defineWranglerConfig } from "wrangler/experimental-config";

export default defineWranglerConfig({
  types: { generate: false },
  assetsDirectory: ".open-next/assets",
});
