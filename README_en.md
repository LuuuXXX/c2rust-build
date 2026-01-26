# c2rust-build

C project build execution tool for c2rust workflow.

## Overview

`c2rust-build` is a command-line tool that executes build commands for C projects, tracks compiler invocations, preprocesses C files, and saves configuration using `c2rust-config`. This tool is part of the c2rust workflow for managing C to Rust translations.

Key features:
- **Real-time Output Display**: Shows detailed command execution output (stdout and stderr) in real-time during builds
- **Configuration File Support**: Read parameters from config file to reduce repeated CLI arguments
- **Build Tracking**: Automatically tracks compiler invocations (gcc/clang) during the build process
- **C File Preprocessing**: Runs the C preprocessor (`-E`) on all tracked C files to expand macros
- **Organized Storage**: Saves preprocessed files to `.c2rust/<feature>/c/` preserving directory structure
- **Interactive Module Selection**: Allows users to select which modules to keep after preprocessing
- **Feature Support**: Supports different build configurations via feature flags

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
# First run (with CLI arguments)
c2rust-build build --dir <directory> -- <build-command> [args...]

# Subsequent runs (using saved config)
c2rust-build build
```

The `build` subcommand will:
1. Read parameters from config file (if exists)
2. Override config values with command-line arguments
3. Track the build process to capture compiler invocations (with real-time output display)
4. Preprocess all C files found during the build using the compiler's `-E` flag
5. Save preprocessed files to `.c2rust/<feature>/c/` directory (default feature is "default")
6. Display an interactive module selection UI
7. Save the build configuration to c2rust-config for later use

### Examples

#### First Run with Make Build

```bash
c2rust-build build --dir /path/to/project -- make
```

This will:
- Display real-time output of executing `make` in `/path/to/project`
- Show the command being executed and the directory
- Show command exit code
- Save configuration to `.c2rust/config.toml`

#### Subsequent Runs (Using Saved Config)

```bash
c2rust-build build
```

Automatically uses saved configuration:
- Reads `build.dir` and `build` command from config file
- No need to repeat parameters

#### Override Specific Parameters

```bash
# Override directory, use saved build command
c2rust-build build --dir /different/path

# Override build command, use saved directory
c2rust-build build -- make clean all

# Override both
c2rust-build build --dir /different/path -- make clean all
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
# Setup debug build configuration
c2rust-build build --feature debug --dir /path/to/project -- make DEBUG=1

# Setup release build configuration
c2rust-build build --feature release --dir /path/to/project -- make RELEASE=1

# Later, switch between configurations
c2rust-build build --feature debug    # Uses saved debug config
c2rust-build build --feature release  # Uses saved release config
```

This will save preprocessed files to `.c2rust/debug/c/` or `.c2rust/release/c/`.

#### Using Custom c2rust-config Path

If `c2rust-config` is not in your PATH or you want to use a specific version:

```bash
export C2RUST_CONFIG=/path/to/custom/c2rust-config
c2rust-build build --dir /path/to/project -- make
```

### Command Line Options

- `--dir <directory>`: Directory to execute build command (optional, can be read from config)
- `--feature <name>`: Optional feature name for the configuration (default: "default")
- `--`: Separator between c2rust-build options and the build command
- `<command> [args...]`: The build command and its arguments to execute (optional, can be read from config)

**Note**: Both `--dir` and command arguments are optional if a config file exists. Command-line arguments override config file values.

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
2. **Config Reading**: Reads saved configuration from `.c2rust/config.toml` (if exists)
3. **Parameter Merging**: Command-line arguments override config file values
4. **Build Tracking**: Executes the build command while tracking compiler invocations
   - Displays command being executed and directory in real-time
   - Shows stdout and stderr output in real-time
   - Shows command exit code
   - Uses custom compiler wrapper scripts
   - Generates a `compile_commands.json` file
5. **Preprocessing**: For each tracked C file:
   - Runs the compiler with `-E` flag to expand macros
   - Saves preprocessed output to `.c2rust/<feature>/c/` directory
   - Maintains the original directory structure
6. **Module Selection**: 
   - Groups files by module (based on directory structure)
   - Presents an interactive selection UI
   - Deletes preprocessed files for unselected modules
7. **Configuration Save**: Saves build configuration via `c2rust-config`:
   - `build.dir`: The directory where builds are executed
   - `build`: The full build command string

### Directory Structure

After running `c2rust-build`, you'll have:
```
project/
├── src/
│   ├── module1/
│   │   └── file1.c
│   └── module2/
│       └── file2.c
├── .c2rust/
│   └── <feature>/        # "default" or specified feature
│       └── c/
│           └── src/
│               ├── module1/
│               │   └── file1.c  # preprocessed
│               └── module2/
│                   └── file2.c  # preprocessed
└── compile_commands.json
```

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
- Preprocessing fails for any C file
- The configuration cannot be saved

## Build Tracking

The tool tracks compiler invocations using custom wrapper scripts:

- Creates temporary wrapper scripts for gcc/clang/cc
- Logs compilation commands during the build
- Generates `compile_commands.json` from logs
- Requires a POSIX-compatible shell (bash) to run the wrapper scripts
- On Windows, requires WSL, Git Bash, or similar Unix-like environment

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
