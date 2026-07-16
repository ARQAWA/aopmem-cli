# AOPMem v0.2 benchmark raw evidence

- `run.json`: host, toolchain, binary hashes, source provenance, and sampling contract.
- `corpora/*.json`: deterministic logical corpus counts and SHA-256 manifests.
- `raw/samples.json` and `raw/samples.csv`: every measured wall-clock sample and exact unsupported reason.
- `summary.csv`: median, nearest-rank p95, minimum, and maximum in milliseconds.
- `SHA256SUMS`: integrity hashes for every evidence file except itself.

Disposable SQLite workspaces are not retained. Regenerate them with
`scripts/benchmark_v020.sh`; the corpus manifests prove logical parity between
the peeled v0.1.0-rc3 tag fixture and the v0.2.0-rc1 fixture.
