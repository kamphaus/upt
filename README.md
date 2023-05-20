# upt

![GitHub](https://img.shields.io/github/license/kamphaus/upt)
![GitHub release (latest SemVer)](https://img.shields.io/github/v/release/kamphaus/upt)
![GitHub Workflow Status (with branch)](https://img.shields.io/github/actions/workflow/status/kamphaus/upt/rust.yml?branch=main)
![GitHub top language](https://img.shields.io/github/languages/top/kamphaus/upt)
![Libraries.io dependency status for GitHub repo](https://img.shields.io/librariesio/github/kamphaus/upt)

This is a simple alternative uptime CLI tool written in Rust.

`upt` does not try to be equivalent to the `uptime` binary.<br>
Rather it defaults to printing duration in a human-readable format.

Optionally the time / duration can be printed in ISO 8601 format by specifying the `--iso` flag.

Additionally, it's possible to watch the duration by adding the `--watch` flag.

A special feature is the ability to set the displayed uptime to zero by using the `--reset` flag.
This does not influence the system uptime counter.
This feature persists the timestamp from the moment the reset is performed.
When evaluating the start time it chooses between the system uptime and the persisted timestamp to choose the latest instant.

All this is accomplished by relying on the fantastic Rust ecosystem and using preexisting crates as much as possible
so as not to reinvent the wheel (e.g. using `clap` for CLI argument parsing and `chrono` for time manipulation).

Finally, a robust CI workflow is used to build release binaries for several platforms and architectures.
These release artifacts include [SBOM](https://en.wikipedia.org/wiki/Software_supply_chain) and [SLSA attestation](https://slsa.dev/).
