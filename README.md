# c2rust-build

用于 c2rust 工作流的 C 项目构建执行工具。

## 概述

`c2rust-build` 是一个命令行工具，用于执行 C 项目的构建命令、追踪编译器调用、预处理 C 文件，并使用 `c2rust-config` 保存配置。该工具是 c2rust 工作流的一部分，用于管理 C 到 Rust 的转换。

主要功能：
- **实时输出显示**：在构建期间实时显示命令执行的详细输出（stdout 和 stderr）
- **构建追踪**：在构建过程中自动追踪编译器调用（gcc/clang）
- **C 文件预处理**：对所有追踪的 C 文件运行 C 预处理器（`-E`）以展开宏
- **有序存储**：将预处理后的文件保存到 `.c2rust/<feature>/c/` 并保留目录结构
- **特性支持**：通过特性标志支持不同的构建配置
- **配置保存**：将构建配置保存到 `config.toml`

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
c2rust-build build -- <command> [args...]
```

**从旧版本迁移：**
如果您之前使用过 `--build.dir` 和 `--build.cmd` 参数，请注意这些参数已被移除。新的使用方式如下：

旧语法：
```bash
c2rust-build build --build.dir /path/to/project --build.cmd make -j4
```

新语法：
```bash
cd /path/to/project
c2rust-build build -- make -j4
```

主要变化：
- 不再需要 `--build.dir` - 在目标目录中运行命令即可
- 使用 `--` 分隔符替代 `--build.cmd`
- 构建目录会自动保存为相对于项目根目录的路径

`build` 子命令将：
1. 追踪构建过程以捕获编译器调用（实时显示构建输出）
2. 使用编译器的 `-E` 标志预处理构建期间找到的所有 C 文件
3. 将预处理后的文件保存到 `.c2rust/<feature>/c/` 目录（默认特性为 "default"）
4. 将构建配置和检测到的编译器保存到 c2rust-config
5. **自动保存**当前命令执行目录（相对于 `.c2rust` 文件夹所在目录）

### 命令行参数

- `--`：参数分隔符，之后的所有参数都是构建命令及其参数；**当构建命令或其参数以 `-` 开头时，必须使用该分隔符**，其他情况下也推荐始终使用
- `--feature <name>`：配置的可选特性名称（默认："default"）

注意：
- 构建命令会在**当前目录**执行
- 工具会自动保存当前目录（相对于项目根目录）作为 `build.dir`
- 使用 `--` 分隔符来区分 c2rust-build 的参数和构建命令的参数；当构建命令或其参数以 `-` 开头时，使用 `--` 可以避免与 c2rust-build 自身的参数产生歧义

### 示例

#### 运行 Make 构建

```bash
# 在项目根目录执行
c2rust-build build -- make

# 或在指定目录执行（使用 cd 切换到该目录）
cd /path/to/project
c2rust-build build -- make
```

这将：
- 在当前目录下实时显示执行 `make` 的输出
- 显示正在执行的命令和目录
- 显示命令退出状态码
- 自动保存当前目录（相对于 .c2rust 文件夹）
- 保存配置到 c2rust-config

#### 运行自定义构建脚本

```bash
c2rust-build build -- ./build.sh
```

#### 运行 CMake 构建

```bash
# 在 build 目录中执行构建
cd build
c2rust-build build -- cmake --build .
```

#### 使用带参数的构建命令

```bash
c2rust-build build -- make -j4 DEBUG=1
```

#### 使用 CFLAGS 的复杂示例

```bash
c2rust-build build -- make CFLAGS="-O2 -g" target
```

#### 使用特性标志运行构建

您可以指定特性名称来组织不同的构建配置：

```bash
# 使用 debug 构建配置
c2rust-build build --feature debug -- make DEBUG=1

# 使用 release 构建配置
c2rust-build build --feature release -- make RELEASE=1
```

这将把预处理后的文件保存到 `.c2rust/debug/c/` 或 `.c2rust/release/c/`。

#### 使用自定义 c2rust-config 路径

如果 `c2rust-config` 不在 PATH 中，或者您想使用特定版本：

```bash
export C2RUST_CONFIG=/path/to/custom/c2rust-config
c2rust-build build -- make
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
2. **目录检测**：自动检测当前执行目录，并计算相对于项目根目录（.c2rust 所在目录）的路径
3. **构建追踪**：在追踪编译器调用的同时执行构建命令
   - 实时显示执行的命令和目录
   - 实时显示 stdout 和 stderr 输出
   - 显示命令退出状态码
   - 使用自定义编译器包装脚本
   - 生成 `.c2rust/compile_commands.json` 文件
4. **预处理**：对每个追踪的 C 文件：
   - 使用 `-E` 标志运行编译器以展开宏
   - 将预处理输出保存到 `.c2rust/<feature>/c/` 目录（默认为 "default"）
   - 保持原始目录结构
5. **配置保存**：通过 `c2rust-config` 保存构建配置：
   - `build.dir`：构建目录（自动检测，相对于项目根目录）
   - `build.cmd`：完整的构建命令字符串
   - `compiler`：检测到的编译器列表
   - 配置可以关联到特定的特性（通过 `--feature` 参数）

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
build.dir = "." (相对于项目根目录)
build.cmd = "make"
compiler = ["gcc", "clang"]
```

使用特性：
```
build.dir = "build" (用于特性 "debug", 相对于项目根目录)
build.cmd = "make -j4" (用于特性 "debug")
```

## 错误处理

工具将在以下情况下退出并显示错误：
- 在 PATH 中找不到 `c2rust-config`
- 缺少必需的构建命令参数（在 `--` 之后）
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
