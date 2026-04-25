# LLM Agent instructions

## General

* Be concise. No apologies.
* Output complete implementations. No TODOs or placeholders.
* Only JSON. No YAML, TOML, etc.

## Rust: general

* Minimal dependencies. Prefer `std`.
* Avoid `.clone()`. Prefer borrowing of `Arc`.
* Error handling: use `anyhow` in business logic, use `thiserror` for library code, always add context.
* Never use `.unwrap()` and `.expect()` outside of tests. Use `?` propagation.
* Prefer functional iterator patterns over for loops.

## Rust: async

* I/O must be async. Use `tokio`.
* Never block the async runtime. Use `spawn_blocking` for CPU-bound tasks.
* Prefer channels to shared state.

## Rust: docs and logging

* Don't use `println!`. Use the `tracing` crate for all logging.
* Public API must have `///` rustdoc, including an `# Errors` section, if they return a `Result`.
* Be discreet. Only comment on *why* the code is doing something, not *what* it is doing.

## Rust: testing

* A module must include a set of unit tests.
* Place unit tests in a `mod tests` block.
* Place integration tests in the `/tests` directory.

## PRs

* Ask for commit approval after each change. If agreed, commit.
* Use Conventional Commits format.
* Commit title should be a continuation of "When merged, this commit will {title}".
* PR description must include *Why* the changes were made, not *What* the code does.