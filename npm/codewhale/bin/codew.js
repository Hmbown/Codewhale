#!/usr/bin/env node

const { run, reportStartFailure } = require("../scripts/run");

run("codew").catch((error) => {
  reportStartFailure("codew", error);
  process.exit(1);
});
