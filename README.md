# ⚠️ hzrd
> Reads as *"hazard"*

A robust, efficient CTF flag capturing and submission framework written in Rust.

## Overview

`hzrd` is a security-focused framework designed for Capture The Flag (CTF) competitions of the Attack/Defense type.

It automates the process of running exploits against target systems, capturing flags, and submitting them to scoring systems.

With features like parallel execution, persistent storage, and configurable timing, it helps teams maximize their effectiveness during competitions.

## Features

- **Exploit Automation**: Run Python-based exploits against multiple targets simultaneously
- **Flag Detection**: Automatically extract flags from exploit output using customizable regex patterns
- **Flag Submission**: Submit captured flags to scoring servers over TCP (HTTP support coming soon)
- **Flag Database**: Keep track of all captured flags, submission status, and points earned
- **Configurable Timing**: Set periodic attack intervals with optional random delays, to help avoiding detection by time-based fingerprinting
- **Test Mode**: Mark specific targets as "NOP" to test exploits without risk of revealing your payloads

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/hzrd.git
cd hzrd

# Build the project
cargo build --release

# The binary will be available at
./target/release/hzrd

# Symlink it to your `.local/bin` folder
mkdir -p ~/.local/bin
ln -s ./target/release/hzrd ~/.local/bin/hzrd

# Add the folder to your PATH
echo 'export PATH="$PATH:$HOME/.local/bin"' >> ~/.bashrc
source ~/.bashrc

# Now you can call `hzrd` from anywhere!
```

## Usage

First, create a configuration file by copying the example:

```bash
cp hzrd.example.toml hzrd.toml
```

You can place it:
- In your current directory (`./hzrd.toml`)
- In your home directory (`~/.config/hzrd/hzrd.toml`)

Edit the configuration file to match your CTF environment.

To override any value at runtime you can either use CLI arguments, or environment variables. The order of precedence is as follows:
1. CLI arguments
2. Environment variables
3. Configuration file

### Running Attacks

```bash
# Run attack using the default config locations
./hzrd attack

# Or specify a custom config file
./hzrd attack -c /path/to/config.toml
```

## Writing Exploits

`hzrd` expects Python exploit scripts with an `exploit(ip)` function that takes the target IP as an argument:

```python
#!/usr/bin/env python3
from pwn import *

def exploit(ip):
    # Connect to the vulnerable service
    r = remote(ip, 1337)

    # Run your exploit...
    r.sendline(b"PAYLOAD")

    # Print captured flags to stdout
    # hzrd will automatically extract them using the flag regex
    print(r.recvall().decode())

    r.close()
```

It is **not** necessary to return anything from this function, as `hzrd` will capture the `stdout` stream and run the configured regex expression on it to extract flags.

## Configuration

The `hzrd.toml` file contains the following sections:

- `attacker`: Main configuration for the attack process
  - `exploit`: Path to exploit script, or directory of scripts
  - `flag`: Regular expression to identify flags in exploit output
  - `loop`: Configuration for periodic execution
    - `every`: Seconds between attack iterations
    - `random`: Maximum random delay (in seconds) before attacks

- `attacker.teams`: Target information
  - Each team entry contains an IP address and optional settings

- `submitter`: Configuration for the flag submission process
  - `type`: Submission method (`tcp` or `http`)
  - `database`: SQLite database configuration
  - `config`: Protocol-specific configurations
  - `submitter.config.tcp`: TCP-based submission servers
    - `host`: Hostname or IP address of the submission server
    - `port`: Port number of the submission server
    - `token`: Authentication token for the submission server

## Example Configuration

```toml
[attacker]
exploit = "./exploits/working.py"
flag = "ptm[A-Z0-9]{28}="

[attacker.loop]
every = 120
random = 10

[attacker.teams]

[attacker.teams.team-one]
ip = "10.66.1.1"

[attacker.teams.team-two]
ip = "10.66.2.1"

[attacker.teams.nop]
ip = "10.66.9.1"
nop = true

[submitter]
type = "tcp"

[submitter.database]
file = "flags.sqlite"

[submitter.config.tcp]
host = "130.192.5.212"
port = 7777
token = "hzrd"
```

## License

MIT License. See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome (and highly appreciated!) Feel free to submit pull requests or create issues for bugs and feature requests.
