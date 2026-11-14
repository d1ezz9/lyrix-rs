# lyrix

Song lyrics in your terminal: synchronized lines for the currently playing track, or plain lyrics you can scroll through. Lyrics are fetched from [LRCLIB](https://lrclib.net/); the current track and playback position are read via `playerctl` (MPRIS).

## Installation

Requirements: Linux, Rust/Cargo, and `playerctl`. By default, the installer builds an optimized release binary and installs it to `~/.local/bin/lyrix`, without requiring root access. If `playerctl` is missing, the installer asks for permission and attempts to install it using the detected package manager.

```bash
./install.sh
```

Useful options:

```bash
./install.sh --yes                    # install playerctl automatically if needed
./install.sh --skip-deps              # skip checking/installing system dependencies
./install.sh --prefix /opt/lyrix       # install to /opt/lyrix/bin/lyrix
./install.sh --system                  # install to /usr/local/bin/lyrix
./uninstall.sh                         # remove the user installation
./uninstall.sh --system                # remove the system installation
```

`--yes` allows the installer to invoke the system package manager with administrator privileges to install `playerctl`. The Rust toolchain is not installed automatically. If Cargo is missing, install Rust (for example, with [rustup](https://rustup.rs)) and rerun the installer.

## Usage

```bash
lyrix
lyrix --player spotify
lyrix --offset-ms -500
```

The default synchronization offset is `-300` ms. Use `--offset-ms` to adjust it, or `--interval-ms` to change the player polling interval (100–2000 ms; 250 ms by default).

Keys: `q` or `Esc` to quit; `↑/↓` or `j/k` to scroll plain lyrics; `r` to retry the lyrics lookup.

## Architecture

- `player`: thin adapter around `playerctl`; commands are passed directly, without a shell.
- `lyrics`: a dedicated thread for LRCLIB requests, with an HTTP timeout, per-track cache, and result ranking by artist/title match, preferring synchronized lyrics.
- `app`: non-blocking application loop and track/keyboard state handling.
- `ui`: terminal interface built with `crossterm`, with terminal-state restoration, Unicode-aware wrapping, and scrolling.

Build and run tests:

```bash
cargo build --release
cargo test
```


# Screenshots

<img width="1920" height="1080" alt="image" src="https://github.com/user-attachments/assets/035cbbad-ce55-409a-8637-8ed0cf1318f3" />

