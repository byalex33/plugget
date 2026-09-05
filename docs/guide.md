# Plugget user guide

[Back to the project overview](../README.md)

**The package manager for Minecraft server plugins.**

```console
$ plugget install luckperms
```

[plugget.dev](https://plugget.dev) · Rust · Native CLI · Modrinth

Plugget installs, tracks, checks and updates plugin JARs in a local Minecraft
server directory. It verifies SHA512 hashes, records package identities and
dependencies, and protects files it does not own. It does not start servers or
hot-load plugins.

## Installation

Build from this checkout with stable Rust (1.88 or newer):

```sh
cargo install --path . --locked
plugget --help
```

Or build with `cargo build --release` and place `target/release/plugget`
(`plugget.exe` on Windows) on your PATH. The executable is self-contained; Rust
is not required to run it.

The release workflow prepares ZIP/tar.gz archives and SHA256 checksum files for
Windows x86_64/ARM64, Linux x86_64/ARM64 and macOS Intel/Apple Silicon. After a release
is published, download the archive for your system, verify its SHA256, extract
it and add the binary directory to PATH. Linux binaries built on Ubuntu 24.04
require glibc 2.39 or newer. All release targets require native CI validation
before publication.

No package registry publication has been performed. winget, Homebrew, Scoop,
Chocolatey, AUR and crates.io distribution are future packaging work.

## Quick start

Stop your server, then run these commands **inside its root directory**:

```sh
plugget init
# If detection needs an explicit override:
plugget init --minecraft 1.21.11 --platform paper

plugget search luckperms
plugget info luckperms
plugget install luckperms
plugget list
plugget outdated
plugget update --all
plugget remove luckperms
```

Restart the server after plugin changes. Plugget always warns before mutations
because it cannot reliably determine whether every possible server launch method
is running. It never attempts plugin loading/unloading.

You can skip `init` when the server platform and Minecraft version are detected
unambiguously: `install` creates missing Plugget configuration automatically.

## Commands

| Command | Purpose |
| --- | --- |
| `init [--minecraft VERSION] [--platform PLATFORM]` | Detect server, record configuration, count existing JARs |
| `search QUERY [--limit 1..100]` | Ranked Modrinth search filtered to plugin loaders |
| `info PLUGIN` | Project, authors, compatible version, platforms, dependencies and installed state |
| `install PLUGIN [--version ID_OR_NUMBER] [--prerelease]` | Install an exact slug/ID or explicitly select a search result |
| `list` | Offline managed inventory and separate unmanaged JAR list |
| `outdated` | Check for newer compatible releases |
| `update [PLUGIN] [--prerelease]` | Update one plugin; no name means all |
| `update --all` | Update all managed plugins, reporting partial failures |
| `remove PLUGIN` | Recycle the exact, checksum-verified managed JAR |
| `doctor [--offline]` | Read-only integrity, duplicate, staging, config and network checks |
| `version` | Show Plugget version |

Every command supports `--help`. Global options: `--quiet`, `--verbose`,
`--json`, `--yes`/`-y`, `--no-color`. Output uses ASCII without terminal color,
so it works in redirected logs and older terminals. Verbose mode prints bounded
HTTP diagnostics to stderr. JSON mode disables diagnostics and prompts.

```sh
plugget search "world edit" --limit 20 --json
plugget install chunky --yes
plugget install luckperms --version VERSION_ID --yes
plugget update --all --yes --json
```

Exact project slugs and IDs resolve directly. A fuzzy result always needs
explicit selection, even when there is only one match. `--yes` does not choose
an ambiguous project. In noninteractive/JSON mode, the error lists possible
slugs; rerun with an exact slug or ID. Noninteractive mutations require `--yes`.

`list` is deliberately offline and labels packages as installed, not as
up-to-date without checking. Use `outdated` for current upstream information.
`info` requires a detected or configured platform and Minecraft version.

## Compatibility and dependencies

| Server | Accepted upstream plugin loaders |
| --- | --- |
| Purpur | purpur, paper, spigot, bukkit |
| Paper | paper, spigot, bukkit |
| Spigot | spigot, bukkit |
| Bukkit/CraftBukkit | bukkit |

Minecraft versions match upstream metadata exactly; `1.21.1` does not imply
`1.21.11`. Plugget sorts releases by publication time, not arbitrary plugin
version strings. Stable releases are used by default. `--prerelease` (or config)
explicitly permits beta/alpha releases. Requested versions must still be
compatible. Updating never automatically downgrades to an older publication.

Detection examines known server JAR names, bounded `version.json`/manifest
entries and generated platform configuration filenames. Conflicting or absent
evidence requires an override; Plugget does not infer versions from old logs.
Folia, Fabric, Forge, Vanilla and other unsupported servers are not inferred as
Paper-compatible.

Required Modrinth dependencies are planned recursively, shown in the confirmation,
and installed with the requested plugin. `--yes` approves that plan. Optional
and embedded dependencies are not installed. Cycles, conflicting pinned
versions, declared conflicts and missing external dependency identities fail
before modifying live files. Packages required by other managed plugins cannot
be removed. Removing a parent does not automatically remove its dependencies.

## Configuration and state

```text
.plugget/
  config.toml
  lock.json
  process.lock
  transaction.json        # only while a transaction/recovery is pending
  transaction-*/          # staged downloads and recoverable backups
plugins/
```

```toml
platform = "paper"
minecraft = "1.21.11"
allow_prerelease = false
```

Global defaults live in `config.toml` under `%APPDATA%\plugget` on Windows,
`~/Library/Application Support/plugget` on macOS, or
`${XDG_CONFIG_HOME:-~/.config}/plugget` on Linux. Per-server settings override
global defaults. Explicit `init` options are saved locally. Existing local
configuration is only replaced after confirmation (or `--yes`).

The schema-1 lock records provider, project ID, slug, version ID/number, owned
filename, SHA512, installation/publication timestamps and dependency identities
with exact pins where required. Project IDs are authoritative, not filenames.
Do not hand-edit the lock to adopt unrelated plugins.

API metadata is cached in memory for 30 seconds within one command. Binary
downloads are streamed with a 512 MiB cap and are not kept as an indefinite
download cache. Failed staging files are retained for diagnosis.

## File safety and recovery

* Downloads stay in staging until size, SHA512 and JAR structure are verified.
* Files are published using a same-filesystem atomic no-clobber hard link.
* Unmanaged target collisions, unsafe filenames, symlinks/reparse points and
  modified managed files stop the operation.
* An exclusive OS process lock serializes changes in a server directory.
* A durable journal precedes live changes; atomic lock replacement commits them.
* Required dependencies and a requested install are one transaction. `update
  --all` uses one transaction per requested package and reports partial failures.
* Existing JARs are retained in staging until commit. Failure rolls back; a
  later mutating command recovers an interrupted transaction automatically.
* Obsolete JARs and transaction files go to the OS Recycle Bin/Trash. There is
  **no permanent-delete fallback**. Plugin config/data directories are retained.

If recycling is unavailable (some headless, container or network-filesystem
setups), cleanup fails explicitly and backups/journal remain. A committed change
may already be active; the error says so. Fix Trash availability and rerun a
mutating command. If the lock or journal is invalid, restore a known-good backup
or inspect it manually; Plugget refuses to guess ownership. `doctor` never fixes
or deletes anything. Unjournaled failed downloads may be moved to Trash manually
after confirming no Plugget process is active.

Use a local filesystem supporting atomic rename and hard links. Transactions
protect against process interruption; they do not provide full ACID guarantees
against hardware failure, hostile local users, or concurrent manual edits by
the server/admin. Stop the Minecraft server before changes. Keep normal server
backups.

## JSON and exit codes

JSON mode emits exactly one object on stdout, including on command errors:

```json
{"schema":1,"ok":true,"data":{"name":"Plugget","version":"0.1.0"}}
```

```json
{"schema":1,"ok":false,"error":{"message":"...","exit_code":1}}
```

Update partial failures use `data.updates` and `data.errors`. Doctor findings
use `data.issues`. Warnings about restarting are represented by
`restart_required` in mutation results. Success envelopes are versioned for
future output evolution.

| Exit | Meaning |
| --- | --- |
| 0 | Success (including no available updates) |
| 1 | Operation, network, safety, configuration or confirmation failure |
| 2 | Invalid CLI arguments |
| 3 | One or more update/check failures; inspect individual results |
| 4 | Doctor found issues |

## Providers

| Provider | Status |
| --- | --- |
| Modrinth | Supported, official v2 API |
| Spigot | Planned |
| Hangar | Planned |
| GitHub Releases | Planned |

Integration follows the official [API overview](https://docs.modrinth.com/api/),
[search](https://docs.modrinth.com/api/operations/searchprojects/),
[project versions](https://docs.modrinth.com/api/operations/getprojectversions/)
and [version](https://docs.modrinth.com/api/operations/getversion/) documentation.
Requests identify `Plugget/VERSION (https://plugget.dev)`. HTTPS is enforced,
redirects are rejected and binary hosts are restricted to Modrinth's CDN.
Timeouts, bounded retries and rate-limit delays are centralized.

## Development and releases

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

`src/cli.rs` and `commands.rs` handle parsing and presentation. `minecraft.rs`
owns detection and loader relationships. `providers/` translates Modrinth data
to common package types; `network.rs` owns HTTP policy. `packages.rs` resolves
versions/dependencies; `state.rs` owns configuration, ownership and recovery.
Tests cover resolver and filesystem behavior, mocked HTTP and executable CLI
flows. Test fixture directories are retained in the OS temp directory rather
than permanently deleted; they can be moved to Trash after inspection.

CI checks Windows, macOS and Linux. A `vX.Y.Z` tag must match Cargo.toml; the
release workflow verifies, builds native archives and checksums, and creates a
**draft** GitHub Release for review. No release has been published by creating
this repository. Signing/notarization and registry manifests remain release
operations requiring maintainer setup.

For v0.2, prioritize hash-based adoption of existing plugins with explicit
identity confirmation. `import`, declarative `plugget.toml`/`sync`, remote
management and additional providers are intentionally outside v0.1.

Plugget is not affiliated with Mojang, Microsoft, PaperMC, or Modrinth.
Plugin compatibility metadata comes from upstream authors and cannot be
guaranteed.
