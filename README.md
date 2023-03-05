# motd-rs

Dynamic MOTD generation, written in rust. Inspired by Ubuntu's [update-motd](https://manpages.ubuntu.com/manpages/latest/en/man5/update-motd.5.html).

---

## Best Practices

_Based on [Ubuntu Manpages/update-motd](https://manpages.ubuntu.com/manpages/latest/en/man5/update-motd.5.html)_

MOTD fragments must be scripts in `<PREFIX>/etc/update-motd.d/`, must be executable, and must emit information on standard out.

Scripts should be named named NN-xxxxxx where NN is a two digit number indicating their position in the MOTD, and xxxxxx is an appropriate name for the script.

Scripts must not have filename extensions, per run-parts(8) --lsbsysinit instructions.

Users should add scripts directly into `<PREFIX>/etc/update-motd.d/`, rather than symlinks to other scripts, such that administrators can modify or remove these scripts and upgrades will not wipe the local changes. Consider using a simple shell script that simply calls exec on the external utility.

Long running operations (such as network calls) or resource intensive scripts should cache output, and only update that output if it is deemed expired. For instance:

```sh
#!/bin/sh
out=/var/run/foo
script="w3m -dump http://news.google.com/"
if [ -f "$out" ]; then
    # Output exists, print it
    echo
    cat "$out"
    # See if it's expired, and background update
    lastrun=$(stat -c %Y "$out") || lastrun=0
    expiration=$(expr $lastrun + 86400)
    if [ $(date +%s) -ge $expiration ]; then
        $script > "$out" &
    fi
else
    # No cache at all, so update in the background
    $script > "$out" &
fi
```

Scripts should emit a blank line before output, and end with a newline character. For instance:
```sh
#!/bin/sh
echo
lsb-release -a
```

## Environment

The `PREFIX` environment variable describes the the path prefix, whichs is combined with the `/etc/update-motd.d/` resulting in the directory containing MOTD fragments. Defaults to `/`, resulting in `/etc/update-motd.d/` as in Ubuntu.

To customize this, for example to use the macOS (with intel) standard prefix `/usr/local`, compile with the `PREFIX` set:
```sh
PREFIX="/usr/local" cargo build
```

Or more genenrally, to use the homebrew prefix for a platfor, compile with:
```sh
PREFIX="$(brew --prefix)" cargo build
```

## Installation / Usage

Build and install `motd` with cargo, and create the MOTD directory
```sh
cargo install --path .
# Or, with a custom PREFIX, e.g. for macOS (with homebrew)
# PREFIX="$(brew --prefix)" cargo install --path .
mkdir -p $(motd --path)
```

Then, add `motd` to your profile (for interactive login shells).
```sh
# E.g. for bash
echo -e "# Dynamic MOTD\n[[ -e \"$(which motd)\" ]] && $(which motd)" >>  ~/.bash_profile
```

### Tools

Additional tools are included that provide platform-specific capabilities. Currently only [macOS](tools/macOS/) tools are available.

Build and install these tools with cargo:
```sh
# on macOS
cargo install --path tools/macOS
# Or, with a custom PREFIX, e.g. for macOS (with homebrew)
# PREFIX="$(brew --prefix)" cargo install --path tools/macOS
```

### Sample MOTD Fragments
A few basic MOTD fragments are provided, currently for [ubuntu](samples/ubuntu/) and [macOS](samples/macOS/).

To include some these samples, copy the included `/samples/<os>` to `<PREFIX>/etc/update-motd.d/`:
```sh
# On macOS
install -D -m 0755 samples/macOS/* -t $(motd --path)
# On Ubuntu
install -D -m 0755 samples/ubuntu/* -t $(motd --path)
```

## LICENSE

This program is licensed under the [GNU General Public License v3.0 or later](https://spdx.org/licenses/GPL-3.0-or-later.html) license. See [LICENSE](LICENSE) for full license text.