# Windows CI test scheduling

Requested by hosted evidence 2026-10-13. Approved by the design lead in this session.
Dev7 Linux passed; the Windows job corrected previous fixtures but timed out in two unchanged
PowerShell lifecycle probes while other native/tool tests were running concurrently.

- Set Windows CI test-thread count to one for the required frontend/root suites. Keep existing
  process, output and shutdown deadlines unchanged; do not turn failures into skips or retries
  inside tests. Treat serialization as scheduling isolation, not proof the backend has no defect.
- Retain explicit byte-exact sustained-output, process-tree cleanup and multi-session/window
  acceptance. Keep recording local parallel-suite evidence separately, and investigate repeated
  failures if a serialized run still fails. No runtime/backend dependency change is proposed.
- Record the observed failures and the chosen schedule in release/decision notes, and verify the
  hosted run/assets before claiming publication. A single unchanged-job rerun may assess a
  transient failure; it does not permit replacing a published tag or concealing a failing test.
