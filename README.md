# c2rust-build

C project build execution tool for c2rust workflow.

## Overview

`c2rust-build` is a command-line tool that executes build commands for C projects and automatically saves the configuration using `c2rust-config`. This tool is part of the c2rust workflow for managing C to Rust translations.

## Installation

### From Source

```bash
cargo install --path .
```

Or build locally:

```bash
cargo build --release
# Binary will be in target/release/c2rust-build
```

## Prerequisites

This tool requires `c2rust-config` to be installed. Install it from:
https://github.com/LuuuXXX/c2rust-config

### Environment Variables

- `C2RUST_CONFIG`: Optional. Path to the c2rust-config binary. If not set, the tool will look for `c2rust-config` in your PATH.

## Usage

### Basic Command

```bash
c2rust-build build --dir <directory> -- <build-command> [args...]
```

The `build` subcommand will:
1. Execute the specified build command in the specified directory
2. Save the build configuration to c2rust-config for later use

### Examples

#### Running Make Build

```bash
c2rust-build build --dir /path/to/project -- make
```

#### Running Custom Build Script

```bash
c2rust-build build --dir . -- ./build.sh
```

#### Running Build with CMake

```bash
c2rust-build build --dir build -- cmake --build .
```

#### Running Build with Feature Flag

You can specify a feature name to organize different build configurations:

```bash
c2rust-build build --feature debug --dir /path/to/project -- make -j4
```

#### Using Custom c2rust-config Path

If `c2rust-config` is not in your PATH or you want to use a specific version:

```bash
export C2RUST_CONFIG=/path/to/custom/c2rust-config
c2rust-build build --dir /path/to/project -- make
```

### Command Line Options

- `--dir <directory>`: Directory to execute build command (required)
- `--feature <name>`: Optional feature name for the configuration (default: "default")
- `--`: Separator between c2rust-build options and the build command
- `<command> [args...]`: The build command and its arguments to execute

### Help

Get general help:

```bash
c2rust-build --help
```

Get help for the build subcommand:

```bash
c2rust-build build --help
```

## How It Works

1. **Validation**: Checks if `c2rust-config` is installed
2. **Execution**: Runs the specified build command in the specified directory
3. **Configuration**: Saves two configuration values:
   - `build.dir`: The directory where builds are executed
   - `build`: The full build command string

## Configuration Storage

The tool uses `c2rust-config` to store build configurations. These can be retrieved later by other c2rust tools.

Example stored configuration:
```
build.dir = "/path/to/project"
build = "make"
```

With a feature:
```
build.dir = "/path/to/project" (for feature "debug")
build = "make -j4" (for feature "debug")
```

## Error Handling

The tool will exit with an error if:
- `c2rust-config` is not found in PATH
- The build command fails to execute
- The configuration cannot be saved

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

Note: Some integration tests may fail if `c2rust-config` is not installed.

### Running Unit Tests Only

```bash
cargo test --lib
```

## License

This project is part of the c2rust ecosystem.

## Related Projects

- [c2rust-config](https://github.com/LuuuXXX/c2rust-config) - Configuration management tool
- [c2rust-test](https://github.com/LuuuXXX/c2rust-test) - Test execution tool
- [c2rust-clean](https://github.com/LuuuXXX/c2rust-clean) - Build artifact cleaning tool

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.