# Website build dependency mitigation

The full npm audit currently reports GHSA-vfj7-8cjw-p6xm in `braces` 3.0.3
and its build-tool dependents. The upstream advisory has no patched release.
We retain the upstream package name, version and licence, and the complete raw
audit report. This is a local mitigation, not an upstream fixed version or a
claim of zero upstream findings.

`npm ci` runs `scripts/patch-braces.mjs`. It verifies the reviewed original or
patched SHA-256 of all four affected source files before writing any, limits
the parser's structural stack and every AST walker to 64 levels, and refuses
an unknown version or changed source. The normal expansion, compilation,
escaping and length limits remain intact. No package is published or replaced.

`npm run audit:dependencies` verifies all four installed patched hashes before
auditing the complete lock. Only this exact advisory and dependencies whose
entire advisory chain ends in it can be classified as mitigated. A second
nested copy, another advisory, an incomplete audit, or an unapplied/changed
patch fails the check. CI saves the raw report, including the mitigated
upstream findings. Installation with `--ignore-scripts` does not apply the
mitigation and cannot pass this gate.

After an upstream fix becomes available, upgrade and verify it, then remove
the installation patch, its audit classification and tests in the same slice.
Never broaden the admitted advisory list to get a passing check.

References: [upstream advisory](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm)
and [upstream issue and recommended depth guard](https://github.com/micromatch/braces/issues/70).
