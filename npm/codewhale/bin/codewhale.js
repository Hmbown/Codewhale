#!/usr/bin/env node

const { runCodeWhale, reportStartFailure } = require("../scripts/run");

runCodeWhale().catch((error) => {
  reportStartFailure("codewhale", error);
  process.exit(1);
});
