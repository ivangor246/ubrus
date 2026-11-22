# Working in Ubrus

Run one Rust command at a time. Wait for its exit before starting another, including checks, tests and formatting.
Use the root Makefile to serialize Cargo and keep compilation to one job.

Read [Architecture](docs/architecture.md) for changes to desktop integration, playback or saved state.
Read [Development](docs/development.md) for build dependencies, verification and packaging.
Match the language of the surrounding comments or documentation.
When UI text changes, update every supported locale and keep message placeholders in sync.
