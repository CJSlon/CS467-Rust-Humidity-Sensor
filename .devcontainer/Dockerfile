# Use Microsoft's official Debian 12 with rust pre-installed
# We're not running Linux on the RPi for our project. This is just 
# specifying the dev container OS. The actual RPi code will be
# compiled to run on the bare metal
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libusb-1.0-0-dev \
    libudev-dev \
    gcc-arm-none-eabi \
    libnewlib-arm-none-eabi \
    cmake \
    minicom

# This is necessary for converting elf files into
# RPi-compatible uf2 files after compiling
# It's a tool though, not a Rust dependency
# Also note that since this is installed in the Dockerfile it will
# be owned by the root user. In order to not cause permissions problems with
# the default "vscode" nonroot user the binar is placed in usr/local and
# the registry info is just deleted. When cargo build is run in the container
# a new registry will be created that's owned by the nonroot user
# This is just one approach to this situation, not necessarily the best one 
RUN cargo install elf2uf2-rs --root /usr/local \
    && rm -rf /usr/local/cargo/registry \
    && rm -rf /usr/local/cargo/git