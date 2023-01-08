# Ser2Tcp-Tester

[![codecov](https://codecov.io/gh/shuanglengyunji/ser2tcp-tester/branch/master/graph/badge.svg?token=6BITB8YX3S)](https://codecov.io/gh/shuanglengyunji/ser2tcp-tester)

A speed tester for Transparent Transmission (TT) between tcp and serial port.

# Usage
```
> ser2tcp-tester --help
Speed tester for transparent transmission between tcp and serial port

Usage: ser2tcp-tester [TYPE:DEVICE] [TYPE:DEVICE]

Arguments:
  [TYPE:DEVICE] [TYPE:DEVICE]  Serial device: serial:/dev/ttyUSB0:115200 (Linux) or serial:COM1:115200 (Windows),
                               TCP server: tcp:192.168.7.1:7
                               Echo mode: use "echo" in place of the second device

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```
# Dependency

For local build on Ubuntu 22.04, install dependency with command: 
```
apt install libudev-dev pkg-config
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