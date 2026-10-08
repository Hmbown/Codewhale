// Node 22.18+ can import this dependency-free TypeScript contract directly.
import { writeFileSync } from "node:fs";
import { CWC_PRODUCT_SCHEMA } from "../src/schema.ts";

writeFileSync(
  new URL("../schema/cwc-product-v3.schema.json", import.meta.url),
  `${JSON.stringify(CWC_PRODUCT_SCHEMA, null, 2)}\n`,
);
