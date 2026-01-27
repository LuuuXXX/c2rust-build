# c2rust-build

用于 c2rust 工作流的 C 项目构建执行工具。

## 概述

`c2rust-build` 是一个命令行工具，用于执行 C 项目的构建命令、追踪编译器调用、预处理 C 文件，并使用 `c2rust-config` 保存配置。该工具是 c2rust 工作流的一部分，用于管理 C 到 Rust 的转换。

主要功能：
- **实时输出显示**：在构建期间实时显示命令执行的详细输出（stdout 和 stderr）
- **构建追踪**：在构建过程中自动追踪编译器调用（gcc/clang）
- **C 文件预处理**：对所有追踪的 C 文件运行 C 预处理器（`-E`）以展开宏
- **有序存储**：将预处理后的文件保存到 `.c2rust/<feature>/c/` 并保留目录结构
- **交互式模块选择**：允许用户选择预处理后要保留的模块
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

该工具需要安装 `c2rust-config`。从以下地址安装：
https://github.com/LuuuXXX/c2rust-config

### 环境变量

- `C2RUST_CONFIG`：可选。c2rust-config 二进制文件的路径。如果未设置，工具将在 PATH 中查找 `c2rust-config`。

## 使用方法

### 基本命令

```bash
c2rust-build build --dir <directory> -- <build-command> [args...]
```

`build` 子命令将：
1. 追踪构建过程以捕获编译器调用（实时显示构建输出）
2. 使用编译器的 `-E` 标志预处理构建期间找到的所有 C 文件
3. 将预处理后的文件保存到 `.c2rust/<feature>/c/` 目录（默认特性为 "default"）
4. 显示交互式模块选择界面
5. 将构建配置保存到 c2rust-config 以供后续使用

### 命令行参数

- `--dir <directory>`：执行构建命令的目录（**必需**）
- `--feature <name>`：配置的可选特性名称（默认："default"）
- `--`：c2rust-build 选项与构建命令之间的分隔符
- `<command> [args...]`：要执行的构建命令及其参数（**必需**）

### 示例

#### 运行 Make 构建

```bash
c2rust-build build --dir /path/to/project -- make
```

这将：
- 在 `/path/to/project` 目录下实时显示执行 `make` 的输出
- 显示正在执行的命令和目录
- 显示命令退出状态码
- 保存配置到 c2rust-config

#### 运行自定义构建脚本

```bash
c2rust-build build --dir . -- ./build.sh
```

#### 运行 CMake 构建

```bash
c2rust-build build --dir build -- cmake --build .
```

#### 使用特性标志运行构建

您可以指定特性名称来组织不同的构建配置：

```bash
# 使用 debug 构建配置
c2rust-build build --feature debug --dir /path/to/project -- make DEBUG=1

# 使用 release 构建配置
c2rust-build build --feature release --dir /path/to/project -- make RELEASE=1
```

这将把预处理后的文件保存到 `.c2rust/debug/c/` 或 `.c2rust/release/c/`。

#### 使用自定义 c2rust-config 路径

如果 `c2rust-config` 不在 PATH 中，或者您想使用特定版本：

```bash
export C2RUST_CONFIG=/path/to/custom/c2rust-config
c2rust-build build --dir /path/to/project -- make
```

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

1. **验证**：检查 `c2rust-config` 是否已安装
2. **参数验证**：确认必需的 `--dir` 和构建命令参数已提供
3. **构建追踪**：在追踪编译器调用的同时执行构建命令
   - 实时显示执行的命令和目录
   - 实时显示 stdout 和 stderr 输出
   - 显示命令退出状态码
   - 使用自定义编译器包装脚本
   - 生成 `.c2rust/compile_commands.json` 文件
4. **预处理**：对每个追踪的 C 文件：
   - 使用 `-E` 标志运行编译器以展开宏
   - 将预处理输出保存到 `.c2rust/<feature>/c/` 目录
   - 保持原始目录结构
5. **模块选择**：
   - 按模块分组文件（基于目录结构）
   - 提供交互式选择界面
   - 删除未选择模块的预处理文件
6. **配置保存**：通过 `c2rust-config` 保存构建配置：
   - `build.dir`：执行构建的目录
   - `build.cmd`：完整的构建命令字符串

### 目录结构

运行 `c2rust-build` 后，您将得到：
```
project/
├── src/
│   ├── module1/
│   │   └── file1.c
│   └── module2/
│       └── file2.c
└── .c2rust/
    ├── compile_commands.json  # 编译命令数据库
    └── <feature>/             # "default" 或指定的特性
        └── c/
            └── src/
                ├── module1/
                │   └── file1.c  # 预处理后
                └── module2/
                    └── file2.c  # 预处理后
```

## 配置存储

该工具使用 `c2rust-config` 存储构建配置。这些配置可以稍后由其他 c2rust 工具检索。

存储配置示例：
```
build.dir = "/path/to/project"
build.cmd = "make"
```

使用特性：
```
build.dir = "/path/to/project" (用于特性 "debug")
build.cmd = "make -j4" (用于特性 "debug")
```

## 错误处理

工具将在以下情况下退出并显示错误：
- 在 PATH 中找不到 `c2rust-config`
- 缺少必需的命令行参数（`--dir` 或构建命令）
- 构建命令执行失败
- 任何 C 文件的预处理失败
- 无法保存配置

## 构建追踪

该工具使用自定义包装脚本追踪编译器调用：

- 为 gcc/clang/cc 创建临时包装脚本
- 在构建期间记录编译命令
- 从日志生成 `.c2rust/compile_commands.json`
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

注意：如果未安装 `c2rust-config`，一些集成测试可能会失败。

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

欢迎贡献！请随时提交问题或拉取请求。
