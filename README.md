# Ser2Tcp-Tester

[![codecov](https://codecov.io/gh/shuanglengyunji/ser2tcp-tester/branch/master/graph/badge.svg?token=6BITB8YX3S)](https://codecov.io/gh/shuanglengyunji/ser2tcp-tester)

A speed tester for Transparent Transmission (TT) between tcp and serial port.

# Usage
```
> ser2tcp-tester --help
Speed tester for transparent transmission between tcp and serial port

Usage: ser2tcp-tester --device <TYPE:DEVICE> <TYPE:DEVICE or echo>

Options:
  -d, --device <TYPE:DEVICE> <TYPE:DEVICE or echo>  Serial port: serial:/dev/ttyUSB0:115200 (Linux) or serial:COM1:115200 (Windows),
                                                    TCP: tcp:192.168.7.1:8000 for tcp server
                                                    Echo mode: use "echo" in place of the second device
  -h, --help                                        Print help information
  -V, --version                                     Print version information
```

# Cross compile

```
Prerequisite:
1. docker 
2. cross: cargo install cross

Windows:
cross build --release --target x86_64-pc-windows-gnu

Linux:
cross build --release --target x86_64-unknown-linux-gnu
```