### sshdb

Keyboard-first SSH library and launcher TUI. Search, preview, and connect with a soft neon look and minimal chrome.

#### UI at a glance

![Screenshot](https://github.com/user-attachments/assets/03dbf3bc-35da-45e8-af9f-0cd29b468c66)

#### Install
- From source: `cargo install --path .`
- Build & run: `cargo run`
- Homebrew: `brew tap ruphy/sshdb && brew install sshdb` (tap updates on releases)


#### Release process
- Tag `vX.Y.Z` and push; CI builds Linux/macOS (x86_64 + arm64)/Windows artifacts.
- Release notes are generated from git history via `git-cliff` and published with the release.
- A `checksums.txt` is attached to every release; use it to update downstream packages (Homebrew, etc.) without extra commits just to bump SHAs.

#### Keys
- `/` search • `Enter` connect • `c` connect with remote command • `g` quick connect (ssh string)
- `n` new host • `e` edit • `d` delete (confirm) • `y` duplicate host • `u` undo last change • `r` reload config
- `j/k` or arrows move • `C` toggle dry-run • `?` help overlay • `a` about/credits • `q`/`Ctrl+C` quit • `Esc` closes modals/help

#### New host dialog
- Paste an `ssh ... user@host` command _or_ fill the fields; both paths are supported (pasting auto-unpacks the fields).
- Fields: `name`, `host`, `user`, `port`, `key_path`, `bastion` (by host name), `tags`, `options` (space-separated, passed through to ssh), `remote_command` (runs by default), `description`.
- Edit host shows a read-only command preview at the bottom.

#### Quick connect
- Hit `g`, paste a raw `ssh user@host` (or full ssh command). If it’s new, sshdb adds it; if it already exists, it reuses it; either way it connects immediately.

#### Config
- Stored at `~/.sshdb/config.toml` (created empty on first run; no sample hosts).
- `default_key` is used when a host has no `key_path`; if set to `agent` sshdb won’t add `-i`.
- If no key is set and an SSH agent exists (e.g., 1Password), sshdb avoids `-i` so the agent works. Without an agent, it falls back to `~/.ssh/id_ed25519` then `~/.ssh/id_rsa`.
- Backups are written as `config.toml.bak` on save.

#### Notes
- TUI is `ratatui` + `crossterm`; real `ssh` runs outside the overlay.
- Dry-run shows the full command before launching; default is live connects.
