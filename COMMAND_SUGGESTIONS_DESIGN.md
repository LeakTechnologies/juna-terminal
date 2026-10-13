# Conservative command suggestions

Requested 2026-10-13. Awaiting approval under AGENTS.md §3.

- Preserve suggestions already produced by a command or shell. Git itself supplies the example `git statsus` → `status`; Juna must not duplicate it or infer a typo from arbitrary terminal output. Never execute a correction, alter history or enable Git/shell autocorrect automatically.
- Add optional local suggestions only for failed commands reported through an explicit, validated shell-integration boundary, with command arguments and failure type available. Use a bounded known-command catalogue from the relevant installed tool/version. Start with an explicitly limited supported command set; do not guess arbitrary executables, arguments, paths, branches, flags or secret values. Use deterministic edit distance including adjacent transpositions, strict length/distance limits and a unique best match; suppress ambiguous or unsupported cases. No model, network lookup or learned completion is involved.
- Display at most one unobtrusive, non-executing suggestion after the error, with an explicit action to place the corrected command at the prompt for review. Disable in alternate screens/TUIs and suppress when the tool already supplied a suggestion. Verify misspellings, valid unknown subcommands, ambiguity, aliases, quoted arguments, shell differences and hostile output. Do not advertise terminal-wide coverage until shell integration and installed-command discovery are verified. No implementation before approval; any new integration boundary gets its own documented acceptance evidence.

Implementation order remains line numbers, bundled Junafetch/privacy and updates, followed by
CI repair. Native Git suggestion passthrough needs no new terminal feature.
