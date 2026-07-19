# RC7 Stage 01 Handoff

Status: `VERIFIED`

Baseline is exact published RC6 on `main`. Local and `origin/main` equal
`b47ff96681c77aa18a4baa07e030fa2dc78eeb88`; annotated `v0.2.0-rc6` peels to
that commit. The immutable RC6 installer SHA-256 matches the release baseline.

Preserve `.devplan/RC6_RELEASE_PUBLICATION_REPORT.md` as untracked,
post-publication RC6 evidence. Do not include it in RC7.

Field evidence isolates the defect to `install/v0.2/install.ps1` transport:
direct strict-mode access to absent `Exception.Response` masks the original
`System.InvalidOperationException`. Apply count is zero and user data is
unchanged. Native RC6 binary platform check passed.

Next: Stage 02 transport specification and proof harness.
