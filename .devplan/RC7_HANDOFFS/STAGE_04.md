# RC7 Stage 04 Handoff

Status: `VERIFIED`

Both installers now use exact platform-specific known-source hashes for
0.1.0 and RC1–RC6. Exact RC4/RC5/RC6 sources emit no warning. Unknown
compatible RC1–RC6 uses `NONCANONICAL_SOURCE_BINARY`; unknown 0.1.0 may retain
`NONCANONICAL_V010_BINARY`. Actual version/hash remain visible.

Focused transport tests pass 30/30. Installer audit passes 14 groups.
Fresh high-reasoning cumulative audit: P1=0, P2=0.

Next: Stage 05 full regression and isolated macOS proof. Do not claim native
PowerShell 5.1 from Mac.
