# Security policy

## Supported versions

Security fixes target the latest published stable Codewhale release. Please
upgrade before reproducing a report when possible.

| Version | Security fixes |
| --- | --- |
| Latest published stable release | Supported |
| Earlier releases | Upgrade to the latest stable release |
| Unreleased branches and development builds | Best effort; no release support commitment |

## Report a vulnerability privately

Use [GitHub's private vulnerability report form](https://github.com/codewhale-hq/Codewhale/security/advisories/new).
Private vulnerability reporting is enabled for this repository. GitHub sends
the report to repository maintainers through a private security advisory.
Do not open a public issue or pull request containing an unpatched
vulnerability or exploit details.

Include the affected version or commit, operating system, a minimal
reproduction, expected and observed behavior, and the security impact.
Remove credentials, tokens, provider keys, personal data, and unrelated
repository contents. If a credential was exposed, rotate it through its
provider before sharing a redacted reproduction.

## What to expect

Maintainers review reports and discuss reproduction, impact, and remediation
in the private advisory. Response and fix times depend on severity and
available capacity; there is no guaranteed response deadline. Please keep
exploit details private while a fix and disclosure timing are discussed.
When a fix is ready, maintainers use the advisory to coordinate publication
and affected versions. Reports that are ordinary bugs may be redirected to
the public issue tracker after sensitive details are removed.
