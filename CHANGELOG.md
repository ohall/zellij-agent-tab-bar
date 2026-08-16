# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and releases follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial workspace architecture for the `zja` CLI, shared event model, and
  Zellij tab-bar plugin.
- Event-driven status reporting over the `zja.events` Zellij named pipe.
- Automatic directory-based tab names with manual-name overrides.
- Configurable semantic status badges and responsive tab rendering.
- Build, installation, test, audit, and contributor documentation.
- Theme-derived status colors for badges (running, complete, error) with a
  dimmed idle badge, dimmed tab indices, and a bar-colored separator so tabs
  read as chips. Badges that would blend into their tab background fall back
  to the tab's base foreground.
- Default status badges are now Emoji: `💤` idle, `🚀` running, `✅` complete,
  and `❌` error.
- Automatic directory naming now treats any Zellij default `Tab #N` name as
  automatic instead of requiring it to match the display position, so tabs
  left over from closed/re-created tabs get directory names again.
- An opencode integration (`opencode/zja-status`) that publishes an agent's
  lifecycle to the tab bar, packaged as `@ohall/zja-status` on npm. See
  `docs/opencode.md`.

No stable release has been published yet.
