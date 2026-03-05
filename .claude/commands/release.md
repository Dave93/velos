Release a new version of Velos.

Argument: version bump type — "patch" (default), "minor", or "major".

## Steps

1. **Determine the new version:**
   - Read current version from `Cargo.toml` (workspace.package.version)
   - Based on the argument ($ARGUMENTS — defaults to "patch"):
     - `patch`: 0.1.4 → 0.1.5
     - `minor`: 0.1.4 → 0.2.0
     - `major`: 0.1.4 → 1.0.0

2. **Generate changelog:**
   - Run `git log --oneline $(git describe --tags --abbrev=0 2>/dev/null || echo HEAD~20)..HEAD` to get all commits since the last tag
   - Read existing `CHANGELOG.md`
   - Create a new entry at the top (after the header), following Keep a Changelog format:
     - Group commits into: Added, Changed, Fixed, Removed (skip "chore: bump version" commits)
     - Use today's date
     - Write concise, user-facing descriptions (not raw commit messages)
   - Update `CHANGELOG.md` with the new entry prepended

3. **Bump version in all files:**
   - `Cargo.toml` — workspace.package.version
   - `distribution/install.sh` — fallback version in get_latest_version()

4. **Commit, tag, and push:**
   - `git add Cargo.toml distribution/install.sh CHANGELOG.md`
   - Commit with message: `chore: release v{NEW_VERSION}`
   - `git push origin main`
   - `git tag v{NEW_VERSION}`
   - `git push origin v{NEW_VERSION}`

5. **Report:**
   - Print the new version
   - Print the changelog entry
   - Print the GitHub Actions release URL
