#!/usr/bin/env sh
set -eu
python3 generate_audio.py
cargo horizon-mod build
