# Changelog

## 0.17.0
_2026-03-11_

### Highlights

#### Added
- Per-host SSH key selection with a picker backed by `~/.ssh`, plus support for multiple identity files per host.
- A per-host `prefer_public_key_auth` toggle that forces `PreferredAuthentications=publickey`.
- `x` as a new shortcut to copy the selected host's full connection string to the clipboard.

#### Changed
- Host configs now persist `key_paths` instead of a single `key_path`, while remaining backward-compatible with existing configs.
- Host details and command previews now reflect multiple keys and the effective auth preference.

#### Fixed
- The inline key selector now closes correctly with `Esc`, keeps the active item visible when the list scrolls, and preserves cursor alignment in the form.
- When `prefer_public_key_auth` is enabled, conflicting `PreferredAuthentications=...` options are stripped so the toggle actually wins.

---

Release notes are normally generated from git history with `git-cliff` during tagged releases.

- To preview locally: `git cliff --tag <next-version> --output /tmp/release-notes.md`
- GitHub releases attach the generated notes plus platform checksums.
