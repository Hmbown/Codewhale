#!/usr/bin/env bash

# Crates published for each codewhale release, in dependency order.
release_crates=(
  codewhale-build-support
  codewhale-mcp
  codewhale-paths
  codewhale-protocol
  codewhale-release
  codewhale-secrets
  codewhale-state
  codewhale-workflow
  codewhale-workflow-js
  codewhale-execpolicy
  codewhale-hooks
  codewhale-tools
  codewhale-config
  codewhale-cloud-facts
  # Path+version dependency of cli/tui — must publish before those crates.
  codewhale-telemetry
  codewhale-lane
  codewhale-agent
  codewhale-core
  # Prototype command boundary depends on core; future TUI/commands adapters
  # consume it without changing current production dispatch in FEAT-014.
  codewhale-command-contract
  # TUI support crates added in 0.9.13: localization (i18n), models (catalog
  # facade), palette (design tokens). Only tui consumes them, so they sit
  # after core/config/build-support and before tui.
  codewhale-localization
  codewhale-models
  codewhale-palette
  # Scoped memory store; tui's native memory backend. No workspace deps.
  codewhale-memory
  codewhale-tui
  codewhale-app-server
  codewhale-cli
)
