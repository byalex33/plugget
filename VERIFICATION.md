# Plugget v0.1.0 implementation and verification

Verified on 2026-09-05 in Windows x86_64 with Rust/Cargo 1.92.0.

## Architecture and files added

The repository was empty. The implementation adds:

| Files | Responsibility |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Native Rust executable and locked dependencies |
| `src/main.rs`, `src/cli.rs` | CLI parsing, JSON envelopes, terminal output and exit codes |
| `src/commands.rs` | All command workflows and confirmation policy |
| `src/minecraft.rs` | Server detection, exact Minecraft versions, loader compatibility, bounded JAR metadata inspection |
| `src/providers/mod.rs` | Provider interface and common project/version/artifact types |
| `src/providers/modrinth/mod.rs` | Official Modrinth v2 API translation |
| `src/network.rs` | HTTPS, redirect/host policy, retries, rate limits, bounded caching and streamed checksummed downloads |
| `src/packages.rs` | Version selection, dependency planning, installation/removal orchestration |
| `src/state.rs` | Global/local config, authoritative ownership, exclusive process locking, atomic metadata and journal recovery |
| `src/lib.rs` | Testable library entry points |
| `tests/core.rs` | Resolver, metadata, ownership, transaction, failure and recovery checks |
| `tests/modrinth.rs` | Mocked HTTP and complete command workflows |
| `tests/cli.rs` | Executable CLI tests in a temporary server directory |
| `.github/workflows/ci.yml` | Windows/macOS/Linux verification |
| `.github/workflows/release.yml` | Six native targets, archives, SHA256 and draft releases |
| `.gitignore`, `LICENSE`, `README.md`, `VERIFICATION.md` | Repository hygiene, license, user guide and this report |

Implemented commands: `init`, `search`, `info`, `install`, `remove`, `list`,
`outdated`, `update [plugin]`, `update --all`, `doctor`, `version`, and help.
Global quiet/verbose/JSON/yes/no-color options are implemented. Install supports
explicit versions and prerelease opt-in. Detection can auto-initialize an
unambiguous server. Removal uses the OS Recycle Bin and retains plugin data.

## Automated verification

All four requested commands completed successfully on the final code:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

30 tests passed: 2 internal unit tests, 18 core integration tests, 9 mocked-HTTP
tests and 1 executable end-to-end test containing multiple command assertions.
There were no warnings, failed tests or ignored tests.

Coverage includes exact version matching, directional platform compatibility,
stable/prerelease selection, explicit versions, ambiguous files/search results,
schema serialization, filename attacks, atomic replacement, process contention,
ownership, unmanaged collisions, required/optional dependencies, loops, exact
pin conflicts, external dependencies, removal protection, duplicate detection,
malformed HTTP, forbidden redirects/hosts, invalid hashes/JARs, retry/cache
behavior, failed-download preservation, same-filename replacement, interruption
recovery, unavailable Trash, and partial filesystem failure with a locked JAR.
The Unix-specific symlink test is present but was not compiled on Windows.

## Manual live verification

Used the disposable server under `.manual-tests/live`, configured as Paper
1.21.11. No Minecraft server process was started.

* Live Modrinth search returned the exact LuckPerms project.
* `info --json` returned authors and a compatible release, with explicit JSON fields.
* Installed LuckPerms `v5.5.71-bukkit` with SHA512/JAR verification and a lock entry.
* `list`, `outdated`, `update --all` and online `doctor` succeeded.
* Removed the managed JAR through the Recycle Bin and verified an empty inventory.
* Installed older release `v5.5.53-bukkit` (ID `MBSY8toc`).
* `outdated` identified `v5.5.71-bukkit` (ID `b0mk8uS6`) as compatible.
* `update luckperms --yes --json` performed the real upgrade successfully.
* Online `doctor` reported no issues after the upgrade.
* The optimized release executable passed `version`, `info`, `remove` and
  offline `doctor`. The final disposable server has no installed plugin JARs.

The executable is `target/release/plugget.exe`.

## Limits and next steps

* macOS/Linux/ARM64 execution and GitHub Actions have not been run in this local
  Windows session. Native release jobs are configured using the documented
  [GitHub runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
* Release signing/notarization, registry publication and release publication
  have not been performed. The workflow creates a draft release for review.
* Linux release builds target Ubuntu 24.04's glibc; older glibc distributions
  need a source build or a future broader-compatibility release target.
* Installation requires a local filesystem with hard-link/atomic-rename support.
  Missing OS Trash support produces an explicit recovery/cleanup error; there
  is no permanent-delete fallback.
* Running-server detection is a conservative warning, not reliable process
  discovery. Stop the server first. Tests validate package management, not
  plugin behavior inside Minecraft.
* Compatibility depends on upstream metadata. Dependency planning rejects
  conflicting selections rather than backtracking across arbitrary versions.
* Import/adoption, manifests/sync, other providers, and purge are deferred.
* Failed unjournaled downloads and test fixtures are retained for inspection;
  move them to Trash when no Plugget process is using them.

Recommended v0.2 work: hash-based adoption of existing JARs with explicit
identity confirmation, following successful native CI and release validation.
