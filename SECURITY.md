# Security Policy

## Supported Versions

The project is in initial development and has not published a stable release.
Security fixes are applied to the latest code on the default branch. Once
releases begin, this section will list the supported version lines.

## Reporting a Vulnerability

Do not report a suspected vulnerability in a public issue, discussion, pull
request, or terminal transcript.

Use the repository host's private security-advisory flow. Include:

- the affected version or commit;
- the operating system, Zellij version, and installation method;
- a minimal reproduction or proof of concept;
- the expected and observed behavior;
- the potential impact; and
- any suggested mitigation.

If private advisories are unavailable, open a public issue that requests a
private contact channel without including vulnerability details.

Maintainers should acknowledge a complete report within seven days, validate and
triage it, and coordinate a disclosure date with the reporter. Timelines depend
on severity and release complexity; avoid public disclosure until a fix or
mitigation is available.

## Security Boundaries

The plugin treats named-pipe messages, KDL configuration, paths, and Zellij host
events as untrusted. It does not inspect terminal contents, prompts, or agent
conversation data. The CLI launches a command only when the user explicitly
invokes `zja run -- <command>`; it does not download or select agents.

Users should install the plugin and CLI from a trusted source, review shell
configuration before applying it, and keep Zellij and Rust-built artifacts up to
date.
