# c2rust-build

用于 c2rust 工作流的 C 项目构建执行工具。

## 概述

`c2rust-build` 是一个命令行工具，用于执行 C 项目的构建命令、追踪编译器调用、预处理 C 文件，并使用 `c2rust-config` 保存配置。该工具是 c2rust 工作流的一部分，用于管理 C 到 Rust 的转换。

主要功能：
- **实时输出显示**：在构建期间实时显示命令执行的详细输出（stdout 和 stderr）
- **构建追踪**：使用 LD_PRELOAD 钩子库在构建过程中自动追踪编译器调用（支持绝对路径的编译器）
- **C 文件预处理**：使用 clang 对所有追踪的 C 文件运行预处理器（`-E`）以展开宏
- **有序存储**：将预处理后的文件保存到 `.c2rust/<feature>/` 并保留目录结构，使用 `.c2rust` 扩展名
- **特性支持**：通过特性标志支持不同的构建配置
- **配置保存**：将构建配置保存到 `config.toml`

## 安装

### 从 crates.io 安装

```bash
cargo install c2rust-build
```

这将从 [crates.io](https://crates.io/crates/c2rust-build) 安装最新的稳定版本。

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

### 必需的工具

1. **c2rust-config**: 从以下地址安装：
   https://github.com/LuuuXXX/c2rust-config

2. **clang**: 用于预处理 C 文件
   ```bash
   # Ubuntu/Debian
   sudo apt-get install clang
   
   # macOS
   brew install llvm
   
   # Fedora/RHEL
   sudo dnf install clang
   ```

3. **Hook 库**: 用于拦截编译器调用
   ```bash
   cd hook
   make
   export C2RUST_HOOK_LIB=$(pwd)/libhook.so
   ```

### 环境变量

- **C2RUST_HOOK_LIB** (必需): libhook.so 的绝对路径
- **C2RUST_CONFIG** (可选): c2rust-config 二进制文件的路径（默认: "c2rust-config"）
- **C2RUST_CLANG** (可选): clang 二进制文件的路径（默认: "clang"）
- **C2RUST_PROJECT_ROOT** (可选): 项目根目录的路径。如果设置，将直接使用该值作为项目根目录，而不是搜索 .c2rust 目录。通常由上游工具（如工作流编排器）设置

## 设置步骤

### 1. 编译 Hook 库

首先，编译 LD_PRELOAD 钩子库：

```bash
cd c2rust-build/hook
make
```

这将生成 `libhook.so`。

### 2. 设置环境变量

设置钩子库路径：

```bash
export C2RUST_HOOK_LIB=/absolute/path/to/c2rust-build/hook/libhook.so
```

或将其添加到您的 shell 配置文件中：

```bash
echo 'export C2RUST_HOOK_LIB=/absolute/path/to/c2rust-build/hook/libhook.so' >> ~/.bashrc
source ~/.bashrc
```

### 3. 验证设置

检查环境变量是否正确设置：

```bash
echo $C2RUST_HOOK_LIB
# 应该显示: /absolute/path/to/c2rust-build/hook/libhook.so

# 验证文件存在
ls -l $C2RUST_HOOK_LIB
```

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
1. 使用 LD_PRELOAD 钩子库追踪构建过程以捕获编译器调用（实时显示构建输出）
2. 使用 clang 的 `-E` 标志预处理构建期间找到的所有 C 文件
3. 将预处理后的文件保存到 `.c2rust/<feature>/` 目录（默认特性为 "default"），文件名为 `*.c2rust`
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

这将把预处理后的文件保存到 `.c2rust/debug/` 或 `.c2rust/release/`。

#### 使用自定义 clang 路径

如果 `clang` 不在 PATH 中，或者您想使用特定版本：

```bash
export C2RUST_CLANG=/usr/bin/clang-15
c2rust-build build -- make
```

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

1. **验证**：检查 `c2rust-config` 和 `clang` 是否已安装
2. **目录检测**：自动检测当前执行目录，并计算相对于项目根目录（.c2rust 所在目录）的路径
3. **构建追踪**：使用 LD_PRELOAD 钩子库在追踪编译器调用的同时执行构建命令
   - 实时显示执行的命令和目录
   - 实时显示 stdout 和 stderr 输出
   - 显示命令退出状态码
   - 拦截所有编译器调用（包括绝对路径调用）
   - 生成 `.c2rust/compile_commands.json` 文件
   - 保存原始钩子输出到 `.c2rust/compile_output.txt`
4. **预处理**：对每个追踪的 C 文件：
   - 使用 clang 的 `-E` 标志运行预处理器以展开宏
   - 提取相关的预处理标志（-I, -D, -U, -std, -include）
   - 将预处理输出保存到 `.c2rust/<feature>/` 目录（默认为 "default"）
   - 保持原始目录结构，使用 `.c2rust` 扩展名
5. **配置保存**：通过 `c2rust-config` 保存构建配置：
   - `build.dir`：构建目录（自动检测，相对于项目根目录）
   - `build.cmd`：完整的构建命令字符串
   - `compiler`：检测到的编译器列表
   - 配置可以关联到特定的特性（通过 `--feature` 参数）
6. **自动提交**（可选）：如果 `.c2rust` 目录下存在 git 仓库（`.c2rust/.git`），工具会自动提交所有修改：
   - 这是一个 best-effort 操作，任何错误只会记录警告而不会导致流程失败
   - 仅当有实际修改时才会创建提交
   - 提交信息为 "Auto-commit: c2rust-build changes"

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
    ├── compile_commands.json       # 标准编译数据库
    ├── compile_output.txt          # 原始钩子输出
    ├── config.toml                 # 构建配置（由 c2rust-config 管理）
    └── <feature>/                  # "default" 或指定的特性
        └── src/                    # 保留源目录结构
            ├── module1/
            │   └── file1.c2rust  # 预处理后的文件（由 clang）
            └── module2/
                └── file2.c2rust  # 预处理后的文件（由 clang）
```

## Hook 库工作原理

Hook 库 (`libhook.so`) 使用 LD_PRELOAD 机制拦截编译器调用：

1. **拦截机制**：通过 `LD_PRELOAD` 环境变量注入到所有子进程
2. **编译器检测**：拦截 `execve` 系统调用，检测 gcc/clang/cc 调用
3. **信息记录**：记录编译选项、文件路径和工作目录
4. **输出格式**：使用 `---ENTRY---` 分隔符格式化输出
5. **线程安全**：使用文件锁处理并行构建

环境变量：
- `C2RUST_ROOT`: 项目根目录（用于过滤项目内的文件）
- `C2RUST_OUTPUT_FILE`: 输出文件路径

## 配置存储

该工具使用 `c2rust-config` 存储构建配置。这些配置可以稍后由其他 c2rust 工具检索。

存储配置示例：
```
build.dir = "." (相对于项目根目录)
build.cmd = "make"
compiler = ["gcc"]
```

使用特性：
```
build.dir = "build" (用于特性 "debug", 相对于项目根目录)
build.cmd = "make -j4" (用于特性 "debug")
```

## 错误处理

工具将在以下情况下退出并显示错误：
- 在 PATH 中找不到 `c2rust-config`
- 在 PATH 中找不到 `clang`（或 `C2RUST_CLANG` 指定的路径）
- 未设置 `C2RUST_HOOK_LIB` 环境变量
- Hook 库文件不存在
- 缺少必需的构建命令参数（在 `--` 之后）
- 构建命令执行失败
- 任何 C 文件的预处理失败
- 无法保存配置

## 构建追踪

该工具使用 LD_PRELOAD 钩子库追踪编译器调用：

- 创建共享库 (`libhook.so`) 来拦截系统调用
- 通过 `LD_PRELOAD` 注入到构建过程
- 拦截所有编译器调用（gcc/clang/cc），无论是相对路径还是绝对路径
- 在构建期间记录编译命令和选项
- 从日志生成 `.c2rust/compile_commands.json`
- 仅在 Linux 上工作（需要 LD_PRELOAD 支持）
- macOS 和 Windows 支持可能在未来添加

## 开发

### 构建主程序

```bash
cargo build
```

### 构建 Hook 库

```bash
cd hook
make
```

### 运行测试

```bash
cargo test
```

注意：如果未安装 `c2rust-config` 或 `clang`，一些测试可能会失败。

### 仅运行单元测试

```bash
cargo test --lib
```

### 清理 Hook 库

```bash
cd hook
make clean
```

## 系统要求

- **操作系统**: Linux（需要 LD_PRELOAD 支持）
- **编译器**: GCC 或 Clang（用于编译 hook 库）
- **Clang**: 用于预处理 C 文件
- **Rust**: 1.70 或更高版本（用于构建主程序）

## 故障排除

### Hook 库未找到

```
Error: Hook library not found. Set C2RUST_HOOK_LIB environment variable to the path of libhook.so
```

**解决方案**：
```bash
export C2RUST_HOOK_LIB=/absolute/path/to/c2rust-build/hook/libhook.so
```

### Clang 未找到

```
Error: clang not found. Please install clang or set C2RUST_CLANG environment variable
```

**解决方案**：
```bash
# 安装 clang
sudo apt-get install clang

# 或设置自定义路径
export C2RUST_CLANG=/usr/bin/clang-15
```

### 未追踪到编译

如果未追踪到 C 文件编译：
1. 确保 `C2RUST_HOOK_LIB` 已正确设置
2. 验证 hook 库已编译：`ls -l $C2RUST_HOOK_LIB`
3. 检查构建命令是否实际编译了 C 文件
4. 确保在 Linux 上运行（LD_PRELOAD 要求）

## 许可证

此项目是 c2rust 生态系统的一部分。

## 相关项目

- [c2rust-config](https://github.com/LuuuXXX/c2rust-config) - 配置管理工具
- [c2rust-test](https://github.com/LuuuXXX/c2rust-test) - 测试执行工具
- [c2rust-clean](https://github.com/LuuuXXX/c2rust-clean) - 构建产物清理工具

## 贡献

欢迎贡献！请随时提交问题或拉取请求。

## 发布流程

本项目使用自动化的 GitHub Actions 工作流发布到 crates.io。

### 发布新版本

1. **更新版本号**
   - 在 `Cargo.toml` 中更新 `version` 字段
   - 遵循 [语义化版本规范](https://semver.org/)

2. **更新 CHANGELOG**
   - 在 `CHANGELOG.md` 中添加新版本的变更记录
   - 记录所有的新功能、bug 修复和重大变更

3. **提交变更**
   ```bash
   git add Cargo.toml CHANGELOG.md
   git commit -m "Bump version to x.y.z"
   git push
   ```

4. **创建版本标签**
   ```bash
   git tag -a vx.y.z -m "Release version x.y.z"
   git push origin vx.y.z
   ```

5. **自动发布**
   - 推送标签后，GitHub Actions 会自动触发发布工作流
   - 工作流会执行以下步骤：
     - 代码格式检查（cargo fmt）
     - 代码检查（cargo clippy）
     - 构建项目（cargo build）
     - 运行测试（cargo test）
     - 验证标签版本与 Cargo.toml 版本一致
     - 执行发布预检（cargo publish --dry-run）
     - 发布到 crates.io（cargo publish）

### 手动触发发布

如果需要手动触发发布流程（例如重新发布失败的版本）：

1. 访问 GitHub Actions 页面
2. 选择 "Publish to crates.io" 工作流
3. 点击 "Run workflow" 按钮
4. 选择要运行的分支
5. 点击 "Run workflow" 确认

### 配置要求

发布流程需要在 GitHub 仓库中配置以下 Secret：

- `CARGO_REGISTRY_TOKEN`: crates.io API token，用于发布验证

仓库管理员可以在仓库设置的 Secrets and variables > Actions 中添加此 Secret。

### 版本策略

- **主版本号 (Major)**: 不兼容的 API 变更
- **次版本号 (Minor)**: 向后兼容的功能新增
- **修订号 (Patch)**: 向后兼容的问题修正

### 故障排除

如果发布失败：

1. 检查 GitHub Actions 工作流日志，查看具体错误信息
2. 确保版本号在 crates.io 上未被使用
3. 确认 `CARGO_REGISTRY_TOKEN` Secret 配置正确且有效
4. 验证所有测试通过且代码符合格式要求
5. 如需要，可以删除标签并重新创建：
   ```bash
   git tag -d vx.y.z
   git push origin :refs/tags/vx.y.z
   # 修复问题后重新创建标签
   git tag -a vx.y.z -m "Release version x.y.z"
   git push origin vx.y.z
   ```
