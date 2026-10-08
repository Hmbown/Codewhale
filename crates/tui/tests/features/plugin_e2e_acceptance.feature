@long-running
# [LONG RUNNING] Plugin discovery and listing acceptance tests. Run with:
# cargo test -p codewhale-tui --test plugin_e2e_acceptance --features long-running-tests -- --test-threads=1
# The same integration target also drives the real distributed binary through a
# sealed PTY for plugin.toml show/trust/enable/revoke, reviewed Skill dispatch,
# and reviewed stdio MCP startup/call/cancellation.
Feature: Plugin discovery and listing

  Scenario: Plugin scripts are discovered from the configured plugin directory
    Given an offline CodeWhale workspace with a configured plugin directory
    And the plugin directory contains:
      | name       | description                | approval |
      | greet      | Say hello to the user      | auto     |
      | audit      | Run a security audit       | required |
      | summarizer | Summarize the given input  | suggest  |
    When the plugin scanner discovers plugins
    Then the scanner should report 3 plugins
    And the scanned plugin "greet" should have "Say hello to the user" as description
    And the scanned plugin "greet" should have "suggest" as approval
    And the scanned plugin "audit" should have "required" as approval
    And the scanned plugin "summarizer" should have "suggest" as approval
    And the scanned plugin "missing-plugin" should not be found

  Scenario: Empty plugin directory reports no plugins
    Given an offline CodeWhale workspace with a configured plugin directory
    And the plugin directory is empty
    When the plugin scanner discovers plugins
    Then the scanner should report 0 plugins

  Scenario: Missing plugin directory reports the path
    Given an offline CodeWhale workspace with a configured plugin directory
    And the plugin directory does not exist
    When the plugin scanner runs
    Then the scanner should report the missing directory path
