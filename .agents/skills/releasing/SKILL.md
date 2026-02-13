---
name: releasing
description: "Commits, tags, and pushes a new release version. Use when asked to release, push, commit and tag, or publish a new version."
---

# Releasing

Handles the full release workflow: bump version, lint, format, commit, tag with changelog, and push.

## Workflow

1. **Determine the new version**
   - Read the current version from `Cargo.toml`
   - Look at the latest git tag to check for drift
   - If the user specified a version, use it. Otherwise, increment the patch version from whichever is higher (Cargo.toml or latest git tag)

2. **Update Cargo.toml**
   - Set the `version` field to the new version
   - Run `cargo check` to update `Cargo.lock`

3. **Run checks**
   - Run `cargo clippy` — fix any warnings related to the current changes
   - Run `cargo fmt` — ensure formatting is clean
   - Run `cargo test` — ensure tests pass

4. **Build the commit message**
   - Use format: `v{VERSION}: {summary}`
   - The summary should be a concise description of changes since the last tag
   - Use `git log --oneline {last_tag}..HEAD` to see what changed

5. **Commit**
   - Stage all changes: `git add -A`
   - Commit with the message from step 4

6. **Tag with changelog**
   - Create an annotated tag: `git tag -a v{VERSION} -m "{changelog}"`
   - The changelog in the tag annotation should list the commit summaries since the previous tag, one per line

7. **Push**
   - `git push && git push --tags`

## Important

- The version in `Cargo.toml` and the git tag MUST always match
- Always use annotated tags (`git tag -a`), never lightweight tags
- Never skip the clippy/fmt/test steps
