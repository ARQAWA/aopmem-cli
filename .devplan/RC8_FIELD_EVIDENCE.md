# RC8 field evidence

## Provenance

- Evidence root:
  `/Users/arkadijcukavin/Downloads/AOPMEM_WINDOWS_FORENSIC_EVIDENCE_20260719-135119`
- Classification: external evidence outside this Git repository.
- Files read recursively: 53.
- Manifest-listed files: 49.
- Actual-only files: `.DS_Store`, `FINAL_SUMMARY.json`, `MANIFEST.json`,
  `experiments/.DS_Store`.
- Manifest verification: all 49 listed SHA-256 values matched.
- Missing manifest files: none.
- Manifest mutation flags:
  `live_mutations_performed=false`,
  `repo_mutations_performed=false`,
  `external_mutations_performed=false`.
- Private raw data was excluded and secrets were redacted by the evidence
  package.

## Sanitized Findings

| Finding | RC8 Decision |
| --- | --- |
| Windows 11 Enterprise x64 build 22631, PowerShell 5.1, non-admin | Official acceptance stays native PowerShell 5.1 and non-admin. |
| `LongPathsEnabled=0` | Recovery filesystem operations use long-path-safe Rust paths. |
| Installed binary was `aopmem 0.2.0-rc4` with expected SHA-256 | RC8 keeps compatible RC4 source classification. |
| Exact recovery parent had 0 stable journals and 0 temp journals | Field failure was not an apply-started recovery. |
| Backup manifest had 1219 entries | This was installer Safety Backup evidence, not RC8 recovery state. |
| Live-vs-backup common file mismatches were 0 | Data did not prove content corruption. |
| Live had at least 250 extra entries | RC7 adopt compared against stale live state. RC8 does not do this. |
| First extra was `workspaces\<workspace>\.mutation.lock` | `.mutation.lock` is explicit ephemeral state. |
| Limited extra classes: `.venv` 207, tools 37, runtimes 2, WAL 1, SHM 1, mutation lock 1, other 1 | `.venv`, tools, and runtimes are persistent and included; WAL/SHM/lock are excluded. |
| Clone lab failed on deep runtime `.venv` path with `WinError 206` | Recovery copy/hash must be long-path-safe with long paths disabled. |
| Unicode, Cyrillic, and spaces passed filesystem probes | RC8 keeps logical UTF-8 relative manifest paths. |
| Direct Python network probe failed, proxy paths passed | RC7 proxy/redirect transport remains. Proxy values are never logged. |
| `workspace_dirs` in one raw inventory field was empty | Not accepted blindly; source and tests define workspace handling. |

## Cause And Effect

RC7 failed because the official installer created a whole-home Safety Backup
and then called `upgrade backup --adopt`. `validate_adoptable_backup` compared
the current live home against the old backup manifest. The live home had
additional operational entries, so the manifest check failed before apply.

RC8 fixes this by separating:

- Safety Backup: installer-owned emergency evidence, never normal adopt input.
- Upgrade Recovery Backup: RC8 binary-owned recovery state with journal schema
  v1 and canonical inventory.

## Evidence-To-Requirements Audit

| Windows finding | RC8 requirement | Owner | Positive test | Negative test | Acceptance check |
| --- | --- | --- | --- | --- | --- |
| No RC7 journal | classify stale pre-apply evidence | `upgrade::recovery` | `recovery_inspect_classifies_orphan_backup_as_stale_pre_apply` | malformed/apply-started inspect test | `upgrade recovery inspect --json` before backup |
| Safety Backup adopt mismatch | no normal installer adopt | installers + CLI | installer audit update success | Safety Backup adopt rejection test | no `upgrade backup --adopt` trace |
| 1219 backup entries | preserve Safety Backup as evidence | installers | installer audit backup retained | rejected normal adopt source | backup path retained in final output |
| 250 extra live entries | canonical inventory | `upgrade::recovery` | inventory preserves tools/runtimes/.venv | excludes lock/WAL/SHM | compare manifest classes |
| `.mutation.lock` extra | ephemeral exclusion | `upgrade::recovery` | inventory exclusion test | live extra classification | no lock in recovery backup |
| WAL/SHM extra | SQLite sidecar exclusion | `upgrade::recovery` | inventory exclusion test | SQLite online backup path | no WAL/SHM in recovery backup |
| `.venv`, tools, runtimes | persistent inclusion | `upgrade::recovery` | inventory inclusion test | no convenience exclusion | files present after update |
| pending audit marker | persistent audit evidence | `audit` + recovery | inventory includes `.pending-snapshot` | no manual delete | marker repaired or retained |
| long path failure | verbatim Windows path IO | `windows_path` + recovery | Windows build + path code | `RECOVERY_LONG_PATH_FAILURE` mapping | native long-path fixture |
| proxy path required | proxy-safe transport | installer | 30 transport tests | invalid proxy tests | proxy bootstrap passes |
| apply not started | safe fresh run | recovery state machine | inspect tests | apply-started blocks | apply attempts exactly one |

## Rejected Or Modified Recommendations

- Did not trust `workspace_dirs=[]`; real source and tests own workspace
  discovery.
- Did not use installer Safety Backup as normal adopt source, even when a
  synthetic adopt fixture can pass.
- Did not copy the forensic package into the repo.
- Did not commit raw live inventory, proxy URI, secrets, `.venv`, binaries from
  evidence, SQLite files, or forensic archive.

## Evidence Files And Hashes

| File | Size | SHA-256 |
| --- | ---: | --- |
| `FINAL_SUMMARY.json` | external | `534510d4c8a9822a366e8f6d5c4c682426818d3b716e9fe175e8608c95f033f6` |
| `MANIFEST.json` | external | `428c4b5384b4f5b21e066e4df72bb0f43e801bb9e8edfb8cf494f944ebf561e4` |
| `.DS_Store` | external | `55da9cca8c54410d9789e8e227b7e93ecf9017ecedc8cac73cf4e97b02661c4b` |
| `experiments/.DS_Store` | external | `5716b98d3ea3cf805c56c6f5e63592ffec559a5a233adb6aded1b2f380a5ea8c` |
| `experiments\adoption-fixtures\home-mutated-after-backup\.aopmem\after-backup-extra.txt` | 7 | `BF8203B7F590C658D576DFF832E6BDBF7719FFA1609A29F47DC7623C29AD1646` |
| `experiments\adoption-fixtures\home-mutated-after-backup\.aopmem\value.txt` | 11 | `C695EEE924ADEA8ECA2AC148FD4FEED2C3F13353CB5988E300A7D6652B176332` |
| `experiments\adoption-fixtures\home-mutated-after-backup\aopmem-home-backup-v0.2.0-rc7-df96ddd2-e4fb-4523-a769-9475288d257e\MANIFEST.sha256` | 80 | `157A5A71DCA208CFBDEE23D5AC567C1129F346FB358B604612E5F2AA2930BCBB` |
| `experiments\adoption-fixtures\home-mutated-after-backup\aopmem-home-backup-v0.2.0-rc7-df96ddd2-e4fb-4523-a769-9475288d257e\value.txt` | 11 | `C695EEE924ADEA8ECA2AC148FD4FEED2C3F13353CB5988E300A7D6652B176332` |
| `experiments\adoption-fixtures\malformed-existing-journal\.aopmem\value.txt` | 11 | `C695EEE924ADEA8ECA2AC148FD4FEED2C3F13353CB5988E300A7D6652B176332` |
| `experiments\adoption-fixtures\malformed-existing-journal\aopmem-home-backup-v0.2.0-rc7-997ef05f-deee-4131-9113-37f3de21dacd\MANIFEST.sha256` | 80 | `157A5A71DCA208CFBDEE23D5AC567C1129F346FB358B604612E5F2AA2930BCBB` |
| `experiments\adoption-fixtures\malformed-existing-journal\aopmem-home-backup-v0.2.0-rc7-997ef05f-deee-4131-9113-37f3de21dacd\value.txt` | 11 | `C695EEE924ADEA8ECA2AC148FD4FEED2C3F13353CB5988E300A7D6652B176332` |
| `experiments\adoption-fixtures\malformed-existing-journal\aopmem-upgrade-recovery-v0.2.0-rc7-01-backup-complete.json` | 10 | `1A16850FBDDCC4EC73EA553DCFFF51FE1D984F26E83B7830A7A161330E08CEF9` |
| `experiments\adoption-fixtures\valid-adopt\.aopmem\value.txt` | 11 | `C695EEE924ADEA8ECA2AC148FD4FEED2C3F13353CB5988E300A7D6652B176332` |
| `experiments\adoption-fixtures\valid-adopt\aopmem-home-backup-v0.2.0-rc7-ef40f68a-8744-4971-9271-0bae67d3c939\MANIFEST.sha256` | 80 | `157A5A71DCA208CFBDEE23D5AC567C1129F346FB358B604612E5F2AA2930BCBB` |
| `experiments\adoption-fixtures\valid-adopt\aopmem-home-backup-v0.2.0-rc7-ef40f68a-8744-4971-9271-0bae67d3c939\value.txt` | 11 | `C695EEE924ADEA8ECA2AC148FD4FEED2C3F13353CB5988E300A7D6652B176332` |
| `experiments\adoption-fixtures\valid-adopt\aopmem-upgrade-recovery-v0.2.0-rc7-01-backup-complete.json` | 348 | `5CE3AEC7BEE939C5805AF19E955B320739FFA0C3C3B2BFB7308345CE58D9F8BA` |
| `experiments\adoption-fixtures\valid-adopt\aopmem-upgrade-recovery-v0.2.0-rc7-02-staged-verified.json` | 437 | `3701B0AEBF9B3DAFEF2D87D696066719B97F33CF0C035A126EDFBEF6BBEFBAF1` |
| `experiments\clone_lab.json` | 40007 | `5E70DCADFE5C4F75C624B68473894FDB3BE96AEC80ED56493D08E26EAF75A09B` |
| `experiments\filesystem\Cyrillic path Тест.txt` | 2 | `2689367B205C16CE32ED4200942B8B8B1E262DFC70D9BC9FBC77C49699A4F1DF` |
| `experiments\filesystem\Unicode path ☃.txt` | 2 | `2689367B205C16CE32ED4200942B8B8B1E262DFC70D9BC9FBC77C49699A4F1DF` |
| `experiments\filesystem\ads.txt` | 6 | `F8BA93752C42985276CAB7DE1169609631ED696B04999D05D49D93169EC05C1A` |
| `experiments\filesystem\create-new.txt` | 1 | `2D711642B726B04401627CA9FBAC32F5C8530FB1903CC4DB02258717921A4881` |
| `experiments\filesystem\lock.txt` | 5 | `8D1F5DCB392DC5CC87686A4646418A0853A6D0E38438093CA71368AC36A5195D` |
| `experiments\filesystem\path with spaces.txt` | 2 | `2689367B205C16CE32ED4200942B8B8B1E262DFC70D9BC9FBC77C49699A4F1DF` |
| `experiments\filesystem\readonly.txt` | 2 | `7EF9EC0CF2C4FACAFDDD03AB96ECA0939D6749B49952BD816F1E0CC6901941D5` |
| `experiments\filesystem\rename-dest.txt` | 1 | `454349E422F05297191EAD13E21D3DB520E5ABEF52055E4964B82FB213F593A1` |
| `experiments\filesystem_matrix.json` | 1918 | `9B9221102BF7BB6BC28929CC02935F16BF09CACCAF09B18DA21B8ABF7E54B5C0` |
| `experiments\powershell51_matrix.json` | 1764 | `EE5D8C12EACDD9ED6D9E8D2B678AC640CD07425AA2013B0E314D993D721075F5` |
| `experiments\recovery_adoption_fixtures.json` | 11498 | `0094FB66952224A4C40C047B09C564F0C2AA3D8FC0498E1CFDFEB342CC19A3CC` |
| `journals\exact_parent_recovery_journals.json` | 122 | `7613DA490D1C3F4082061D72742F53D4FF83CAEFC02D7953B12F0E70D127580B` |
| `journals\live_recovery_candidates.json` | 295492 | `5276E1D0E325DE6EBC5116CAC664215359B660B0F9C48AE574480A97D33DB3C5` |
| `live-metadata\adoptable_backup_diff.json` | 91599 | `FF5F063E5EE42172E1795101AB65EC751547A7EBC92AF74BA35B3B512F40B75F` |
| `live-metadata\live_inventory.json` | 338501 | `89F26E46E74308510F96599AB8225DBB86F951059E941F6DCC528569D6D9D977` |
| `live-metadata\local_rules.json` | 26767 | `247B154154A907863F57C73E95535DF22D9C73C59084452A9A2A1562D566289C` |
| `live-metadata\repo_state.json` | 1029 | `C9A3B6C15983B94C8EFAB44031B38BB6EDF3FCE082A1B979114DC02295804B88` |
| `live-metadata\windows_probe.json` | 7764 | `46B3079E75EE14342AD911072947F2F348CACB669EAB8CBF13530932126D2287` |
| `network\network_matrix_raw.json` | 28592 | `51D5F53F6EE0D8F728BDCEB0BCEF75C707534B5F793E6C5268F325C83811B571` |
| `network\powershell_network_matrix.json` | 5233 | `41D1B4353F12FE21E0E6CEEE36118382DF505AFD1F195324C33C28CD712787BC` |
| `report\AOPMEM_WINDOWS_FORENSIC_REPORT.md` | 1910 | `4F437BAC8D3AF9144F809FD97311FE1F21F7F941B8A11B26997F333C54E4FDFC` |
| `report\CURRENT_BLOCKER_ROOT_CAUSE.md` | 1183 | `26909B4C654DF78997C6C705E14E95E1D45628D47A8C04FB3738A3BF4218DC36` |
| `report\FILESYSTEM_SECURITY_COMPATIBILITY_MATRIX.md` | 1440 | `5F47F95D020FC80CB8F01E23FE5CED938ABF43A6DE8AD14A1C3E21675CF6ED70` |
| `report\FUTURE_FAILURE_SURFACE_MATRIX.md` | 775 | `146B6C48C84747B80E811F00FE022306E524FCC99548633EC78E3D2EEE15FA8A` |
| `report\INSTALLER_STATE_MACHINE.md` | 3382 | `87C4A1CE7A31103E1BF5FDE0DC6B6308AAEEA67FD820C0088948648BB2480361` |
| `report\INSTALLER_WRITE_SET.md` | 9937 | `11EC06BA04C3D261BD5A2A18EB87232D10E9F1A086A21614E1D6277313AE8D1E` |
| `report\LIVE_RECOVERY_JOURNAL_INVENTORY.md` | 894 | `E1711AFA374B307BE2A1516F00C46B8707F051C932EB61B71AD9248B7FE5FAA7` |
| `report\LOCAL_OPERATING_CONSTRAINTS.md` | 21224 | `274FA2A21AE8FFF0E6406883F9612BB33CF9C36B227CDC1CCA05436DF8E33A30` |
| `report\NETWORK_PROXY_REDIRECT_MATRIX.md` | 3158 | `2AB8914A8A855C85E3CC776EB6FF00173D988F24DA4F0A750AF725D97F34E99B` |
| `report\POST_RC8_WINDOWS_ACCEPTANCE_PLAN.md` | 864 | `2AE12D2BA789DAFEE80381B7B53CF96E58C9529FD3B1D592ABE542D16B9DE982` |
| `report\POWERSHELL51_COMPATIBILITY_MATRIX.md` | 935 | `5F006C46619BA6DD7ECD05D2A64D05DBD92A347FB1C7931DC38458176A543519` |
| `report\RC8_IMPLEMENTATION_BRIEF.md` | 2181 | `0467023B28C992E81E304B98FC5C44A5233CF95D6D27BDBE56E47EE37401A332` |
| `report\RECOVERY_JOURNAL_CONTRACT.md` | 1718 | `4506E94FB2DA04E52C0729544FF931884B960C0635513F10272136CC47344078` |
| `report\RECOVERY_JOURNAL_EXPERIMENT_MATRIX.md` | 1342 | `8C9BAC9661D1903EB0E0AE3ECEDCD4EEE4AFD79623F9D20EEA29D61B63EE97C2` |
| `report\WINDOWS_VDI_CAPABILITY_MATRIX.md` | 2216 | `52727223C2864FD08CDA0A417B7176E9297EF63D78BFBC9B1C3CE3F05DB13EBC` |
