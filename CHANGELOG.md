# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Hierarchical file/folder selection with tree structure display
- Support for selecting entire folders (automatically includes all files within)
- Visual indicators in file selection: 📁 for folders, 📄 for files
- Indented tree view showing directory hierarchy
- Recursive folder selection: selecting a parent folder selects all child files
- Batch selection improvements for large projects

### Changed
- File selection UI now displays files organized by directory structure
- Enhanced user experience for selecting multiple related files

## [0.1.0] - 2024-01-01

### Added
- Initial release of c2rust-build
- Build tracking using LD_PRELOAD hook library
- C file preprocessing with clang
- Real-time output display during build
- Automatic directory structure preservation
- Feature-based configuration support
- Integration with c2rust-config for configuration management
- Automatic git commit support for .c2rust directory
- Support for custom clang and c2rust-config paths
- Comprehensive error handling and validation
- Command-line interface with clap
- Automated release workflow with GitHub Actions
- Track compiler calls during build process
- Preprocess C files using clang -E flag
- Save preprocessed files to .c2rust/<feature>/ directory
- Generate compile_commands.json
- Save build configuration via c2rust-config
- Support for multiple build configurations via features
- Real-time build output streaming

[Unreleased]: https://github.com/LuuuXXX/c2rust-build/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LuuuXXX/c2rust-build/releases/tag/v0.1.0
