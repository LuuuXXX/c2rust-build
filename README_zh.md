# c2rust-build

c2rust 工作流的 C 项目构建执行工具。

## 概述

`c2rust-build` 是一个命令行工具，用于执行 C 项目的构建命令、跟踪编译器调用、预处理 C 文件，并使用 `c2rust-config` 保存配置。该工具是 c2rust 工作流的一部分，用于管理 C 到 Rust 的转换。

主要功能：
- **构建追踪**：在构建过程中自动跟踪编译器调用（gcc/clang）
- **C 文件预处理**：对所有被追踪的 C 文件运行 C 预处理器（`-E`）以展开宏
- **有组织的存储**：将预处理后的文件保存到 `.c2rust/<feature>/c/`，保留目录结构
- **交互式模块选择**：允许用户在预处理后选择要保留的模块
- **特性支持**：通过特性标志支持不同的构建配置

## 安装

### 从源码安装

```bash
cargo install --path .
```

或本地构建：

```bash
cargo build --release
# 二进制文件将位于 target/release/c2rust-build
```

## 前置条件

此工具需要安装 `c2rust-config`。从以下位置安装：
https://github.com/LuuuXXX/c2rust-config

### 环境变量

- `C2RUST_CONFIG`：可选。c2rust-config 二进制文件的路径。如果未设置，工具将在 PATH 中查找 `c2rust-config`。

## 使用

### 基本命令

```bash
c2rust-build build --dir <directory> -- <build-command> [args...]
```

`build` 子命令将：
1. 追踪构建过程以捕获编译器调用
2. 使用编译器的 `-E` 标志预处理构建过程中找到的所有 C 文件
3. 将预处理后的文件保存到 `.c2rust/<feature>/c/` 目录（默认特性为 "default"）
4. 显示交互式模块选择界面
5. 将构建配置保存到 c2rust-config 以供后续使用

### 示例

#### 运行 Make 构建

```bash
c2rust-build build --dir /path/to/project -- make
```

#### 运行自定义构建脚本

```bash
c2rust-build build --dir . -- ./build.sh
```

#### 使用 CMake 运行构建

```bash
c2rust-build build --dir build -- cmake --build .
```

#### 使用特性标志运行构建

您可以指定特性名称来组织不同的构建配置：

```bash
c2rust-build build --feature debug --dir /path/to/project -- make -j4
```

这将把预处理后的文件保存到 `.c2rust/debug/c/` 而不是 `.c2rust/default/c/`。

#### 使用自定义 c2rust-config 路径

如果 `c2rust-config` 不在您的 PATH 中，或者您想使用特定版本：

```bash
export C2RUST_CONFIG=/path/to/custom/c2rust-config
c2rust-build build --dir /path/to/project -- make
```

### 命令行选项

- `--dir <directory>`：执行构建命令的目录（必需）
- `--feature <name>`：配置的可选特性名称（默认："default"）
- `--`：c2rust-build 选项和构建命令之间的分隔符
- `<command> [args...]`：要执行的构建命令及其参数

### 帮助

获取常规帮助：

```bash
c2rust-build --help
```

获取 build 子命令的帮助：

```bash
c2rust-build build --help
```

## 工作原理

1. **验证**：检查是否安装了 `c2rust-config`
2. **构建追踪**：在追踪编译器调用的同时执行构建命令
   - 使用自定义编译器包装脚本
   - 生成 `compile_commands.json` 文件
3. **预处理**：对每个被追踪的 C 文件：
   - 使用 `-E` 标志运行编译器以展开宏
   - 将预处理后的输出保存到 `.c2rust/<feature>/c/` 目录
   - 维护原始目录结构
4. **模块选择**：
   - 按模块分组文件（基于目录结构）
   - 呈现交互式选择界面
   - 删除未选择模块的预处理文件
5. **配置**：通过 `c2rust-config` 保存构建配置：
   - `build.dir`：执行构建的目录
   - `build`：完整的构建命令字符串

### 目录结构

运行 `c2rust-build` 后，您将得到：
```
project/
├── src/
│   ├── module1/
│   │   └── file1.c
│   └── module2/
│       └── file2.c
├── .c2rust/
│   └── <feature>/        # "default" 或指定的特性
│       └── c/
│           └── src/
│               ├── module1/
│               │   └── file1.c  # 预处理后
│               └── module2/
│                   └── file2.c  # 预处理后
└── compile_commands.json
```

## 配置存储

该工具使用 `c2rust-config` 存储构建配置。这些配置可以稍后被其他 c2rust 工具检索。

存储配置示例：
```
build.dir = "/path/to/project"
build = "make"
```

使用特性：
```
build.dir = "/path/to/project" (用于特性 "debug")
build = "make -j4" (用于特性 "debug")
```

## 错误处理

以下情况下工具将退出并报错：
- 在 PATH 中找不到 `c2rust-config`
- 构建命令执行失败
- 任何 C 文件的预处理失败
- 无法保存配置

## 构建追踪

该工具使用自定义包装脚本追踪编译器调用：

- 为 gcc/clang/cc 创建临时包装脚本
- 在构建期间记录编译命令
- 从日志生成 `compile_commands.json`
- 需要 POSIX 兼容的 shell（bash）来运行包装脚本
- 在 Windows 上，需要 WSL、Git Bash 或类似的类 Unix 环境

## 开发

### 构建

```bash
cargo build
```

### 运行测试

```bash
cargo test
```

注意：如果未安装 `c2rust-config`，某些集成测试可能会失败。

### 仅运行单元测试

```bash
cargo test --lib
```

## 许可证

此项目是 c2rust 生态系统的一部分。

## 相关项目

- [c2rust-config](https://github.com/LuuuXXX/c2rust-config) - 配置管理工具
- [c2rust-test](https://github.com/LuuuXXX/c2rust-test) - 测试执行工具
- [c2rust-clean](https://github.com/LuuuXXX/c2rust-clean) - 构建产物清理工具

## 贡献

欢迎贡献！请随时提交 issue 或 pull request。
