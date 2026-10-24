# defi-estate-contract

[![Contracts CI](https://github.com/defi-estate/defi-estate-contract/actions/workflows/contracts-ci.yml/badge.svg)](https://github.com/defi-estate/defi-estate-contract/actions/workflows/contracts-ci.yml)

Soroban smart contracts for [defi-estate](https://github.com/defi-estate), a real estate tokenization and rental yield platform on Stellar. One `property-token` and one `yield-distributor` are deployed per property.

| Crate | Purpose |
| --- | --- |
| `contracts/property-token` | capped, whitelist-gated share token for one property (0 decimals) |
| `contracts/yield-distributor` | holds rental income in a payout asset (for example USDC) and lets holders claim it |

Full interface, errors and signing model: [docs/contracts.md](docs/contracts.md).

## Related repositories

- [defi-estate-frontend](https://github.com/defi-estate/defi-estate-frontend): Next.js webapp
- [defi-estate-backend](https://github.com/defi-estate/defi-estate-backend): Elysia API, holds the admin key that whitelists, mints and distributes

## Requirements

Rust stable with the `wasm32v1-none` target (`rustup target add wasm32v1-none`) and the [Stellar CLI](https://developers.stellar.org/docs/tools/cli).

## Commands

```bash
make check                 # fmt, clippy, test, wasm build (same as CI)
cargo test
stellar contract build     # or: cargo build --target wasm32v1-none --release
make fix                   # cargo fmt
```

## Test coverage

CI measures line coverage of the contracts with [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) on every pull request. The job fails if line coverage drops below **85%**, and the generated `lcov.info` report is uploaded as the `coverage-lcov` workflow artifact (Actions → **Contracts CI** → the run).

Run it locally:

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --workspace --lcov --output-path lcov.info
cargo llvm-cov --workspace --summary-only   # human-readable summary
```

## Deploy

```bash
stellar keys generate --global admin --network testnet --fund
./scripts/deploy-contracts.sh admin <property-uuid> "Lekki Garden Shares" LKGS 10000 <usdc-sac-id> testnet
```

Record the printed IDs in `deployments/<network>.json`.

## TypeScript bindings

Every CI run generates TypeScript client bindings for both contracts from the built wasm and uploads them as a workflow artifact named `contract-bindings`. The artifact contains one package per contract:

```
bindings/
├── property-token/
└── yield-distributor/
```

### Download the artifact

From the GitHub UI, open the workflow run for the commit you want (Actions → **Contracts CI** → the run) and download the `contract-bindings` artifact. With the [GitHub CLI](https://cli.github.com/):

```bash
# latest run on the current branch
gh run download --name contract-bindings --dir ./contract-bindings

# or a specific run
gh run download <run-id> --name contract-bindings --dir ./contract-bindings
```

### Generate locally

You can also produce the bindings yourself after a wasm build:

```bash
stellar contract build
stellar contract bindings typescript \
  --wasm target/wasm32v1-none/release/property_token.wasm \
  --output-dir bindings/property-token --overwrite
stellar contract bindings typescript \
  --wasm target/wasm32v1-none/release/yield_distributor.wasm \
  --output-dir bindings/yield-distributor --overwrite
```

### Copy into the other repos

The generated packages are self-contained npm packages. Copy the folder for the contract you need into the consuming repo, for example:

```bash
# frontend: read-only clients for the webapp
cp -R contract-bindings/property-token    ../defi-estate-frontend/src/contracts/property-token
cp -R contract-bindings/yield-distributor ../defi-estate-frontend/src/contracts/yield-distributor

# backend: clients used by the admin API
cp -R contract-bindings/property-token    ../defi-estate-backend/src/contracts/property-token
cp -R contract-bindings/yield-distributor ../defi-estate-backend/src/contracts/yield-distributor
```

Then install and import them like any local package:

```bash
# in the consuming repo, from the directory containing the copied package
npm install ./src/contracts/property-token
```

```ts
import { Client as PropertyTokenClient } from "./contracts/property-token";
import { Client as YieldDistributorClient } from "./contracts/yield-distributor";
```

Commit the copied packages (or vendor them via your package manager) so the frontend and backend stay in sync with the deployed contract version. Regenerate and re-copy whenever the contract interface changes.