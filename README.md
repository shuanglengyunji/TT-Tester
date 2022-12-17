# Ser2Tcp-Tester

[![codecov](https://codecov.io/gh/shuanglengyunji/TT-Tester/branch/master/graph/badge.svg?token=6BITB8YX3S)](https://codecov.io/gh/shuanglengyunji/TT-Tester)

A speed tester for Transparent Transmission (TT) between tcp and serial port.

# Usage
```
> ser2tcp-tester --help
Speed tester for transparent transmission between tcp and serial port

Usage: ser2tcp-tester --serial <DEVICE:BAUD_RATE> --tcp <ADDRESS:PORT>

Options:
  -s, --serial <DEVICE:BAUD_RATE>  Serial port device, for example: /dev/ttyUSB0:115200 (Linux) or COM1:115200 (Windows)
  -t, --tcp <ADDRESS:PORT>         Tcp port, for example: 192.168.7.1:8000
  -h, --help                       Print help information
  -V, --version                    Print version information
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