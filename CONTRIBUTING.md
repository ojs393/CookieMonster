# Contributing

Contributions are welcome — bug fixes, tests, docs, and features.

## License reminder

- Public code is under **PolyForm Noncommercial 1.0.0** (`LICENSE`).
- Commercial / paid use needs a separate license (`COMMERCIAL.md`).
- PRs require accepting the CLA (`CLA.md`).

## Before you open a PR

1. Fork and branch off `main`.
2. Keep changes focused; match existing style.
3. Run tests:

   ```sh
   ./scripts/run_all_tests.sh
   ```

4. In the PR body, include:

   ```text
   I have read and agree to CLA.md for this contribution.
   ```

## Good first areas

- Tests and docs
- Linux/Windows install helpers
- Extension polish (Chromium / Firefox)
- Accessibility and UI clarity

Safari support and OS proxy/CA helpers outside macOS are still
early — call that out in the PR if you touch them.

## Security

Do not file public issues for exploitable vulns that could harm
users’ machines. Contact [@ojs393](https://github.com/ojs393)
privately first.
