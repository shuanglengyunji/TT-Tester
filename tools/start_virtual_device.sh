#!/bin/bash

socat -dd pty,raw,echo=0,link=/tmp/serial1 pty,raw,echo=0,link=/tmp/serial2 &
ser2net -C 2000:raw:0:/tmp/serial1:115200
