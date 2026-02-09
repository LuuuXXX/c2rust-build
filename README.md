# c2rust-build

用于 c2rust 工作流的 C 项目构建执行工具。

## 概述

`c2rust-build` 是一个命令行工具，用于执行 C 项目的构建命令、追踪编译器调用，并使用 `c2rust-config` 保存配置。该工具是 c2rust 工作流的一部分，用于管理 C 到 Rust 的转换。预处理文件由新的 libhook.so 在构建过程中直接生成。

主要功能：
- **实时输出显示**：在构建期间实时显示命令执行的详细输出（stdout 和 stderr）
- **构建追踪**：使用 LD_PRELOAD 钩子库在构建过程中自动追踪编译器调用（支持 gcc/clang/cc，包括绝对路径）
- **预处理文件生成**：新的 libhook.so 在构建过程中直接生成预处理文件到 `.c2rust/<feature>/c/` 目录
- **二进制产物追踪**：自动记录所有构建的二进制文件（静态库、动态库、可执行程序）到 `targets.list`
- **交互式文件选择**：提供用户友好的界面选择需要翻译的预处理文件
- **有序存储**：预处理后的文件保存到 `.c2rust/<feature>/c/` 并保留目录结构，文件名后添加 `.c2rust` 后缀（如 `main.c` → `main.c.c2rust`）或 `.i` 扩展名
- **特性支持**：通过特性标志支持不同的构建配置
- **配置保存**：将构建配置保存到 `config.toml`
- **自动提交**：如果 `.c2rust` 目录下存在 git 仓库，会自动提交所有修改（best-effort 操作）

## 安装

### 从 crates.io 安装

```bash
cargo install c2rust-build
```

这将从 [crates.io](https://crates.io/crates/c2rust-build) 安装最新的稳定版本。（注意：只有在 `c2rust-build` 的首个版本发布到 crates.io 之后，此安装方式才可用。）

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

2. **Hook 库**: 用于拦截编译器调用并生成预处理文件
   ```bash
   cd hook
   make
   export C2RUST_HOOK_LIB=$(pwd)/libhook.so
   ```

**注意**：新版本的 libhook.so 会在构建过程中直接生成预处理文件，无需再安装 clang 进行预处理。

### 环境变量

**用户设置的环境变量：**
- **C2RUST_HOOK_LIB** (必需): libhook.so 的绝对路径
- **C2RUST_CONFIG** (可选): c2rust-config 二进制文件的路径（默认: "c2rust-config"）

**内部使用的环境变量（由工具自动设置）：**
- **C2RUST_ROOT**: 项目根目录的绝对路径（由 c2rust-build 传递给 hook 库，用于过滤项目内的文件）
- **LD_PRELOAD**: 用于注入 hook 库的系统环境变量

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
2. **新的 libhook.so 在构建过程中直接生成预处理文件**到 `.c2rust/<feature>/c/` 目录
3. 收集生成的预处理文件，并提供交互式界面让用户选择需要翻译的文件
4. 将用户选择保存到 `.c2rust/<feature>/selected_files.json`
5. 将构建配置保存到项目配置
6. **自动保存**当前命令执行目录（相对于 `.c2rust` 文件夹所在目录）
7. **自动提交**（如果存在 `.c2rust/.git`）：将所有修改提交到本地 git 仓库

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

#### 文件选择

在构建过程完成后，工具会自动收集生成的预处理文件，并提供交互式界面供您选择：

- 使用 **空格键** 选择/取消选择文件
- 使用 **回车键** 确认选择
- 使用 **ESC 键** 取消操作
- 默认情况下所有文件都被选中

选择的文件列表会保存到 `.c2rust/<feature>/selected_files.json`，供后续翻译步骤使用。

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
2. **项目根目录检测**：
   - 从当前目录向上搜索 `.c2rust` 目录
   - 如果找不到 `.c2rust` 目录，使用当前目录作为项目根目录（在首次运行时 `.c2rust` 目录会在此创建）
3. **目录检测**：自动检测当前执行目录，并计算相对于项目根目录（.c2rust 所在目录）的路径
4. **构建追踪**：使用 LD_PRELOAD 钩子库在追踪编译器调用的同时执行构建命令
   - 实时显示执行的命令和目录
   - 实时显示 stdout 和 stderr 输出
   - 显示命令退出状态码
   - 拦截所有编译器调用（包括绝对路径调用，支持 gcc/clang/cc）
   - **新的 libhook.so 直接生成预处理文件**到 `.c2rust/<feature>/c/` 目录
5. **文件收集与选择**：
   - 遍历 `.c2rust/<feature>/c/` 目录收集所有生成的预处理文件
   - 显示交互式文件选择界面
   - 用户可以选择需要翻译的文件（默认全选）
   - 将选择保存到 `.c2rust/<feature>/selected_files.json`
6. **配置保存**：通过 `c2rust-config` 保存构建配置：
   - `build.dir`：构建目录（自动检测，相对于项目根目录）
   - `build.cmd`：完整的构建命令字符串
   - 配置可以关联到特定的特性（通过 `--feature` 参数）
7. **自动提交**（可选）：如果 `.c2rust` 目录下存在 git 仓库（`.c2rust/.git`），工具会自动提交所有修改：
   - 这是一个 best-effort 操作，任何错误只会记录警告而不会导致流程失败
   - 仅当有实际修改时才会创建提交
   - 提交信息为 "Auto-commit: c2rust-build changes"
   - 自动执行 `git add .` 添加所有变更
   - 如果 git 用户信息未配置，会显示警告但不会失败

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
    ├── config.toml                 # 构建配置（由 c2rust-config 管理）
    ├── .git/                       # 可选：git 仓库（用于自动提交）
    └── <feature>/                  # "default" 或指定的特性
        ├── c/                      # 预处理后的 C 文件目录（由 libhook.so 生成）
        │   ├── targets.list        # 构建的二进制文件列表
        │   └── src/                # 保留源目录结构
        │       ├── module1/
        │       │   └── file1.c.c2rust  # 预处理后的文件（或 .i 文件）
        │       └── module2/
        │           └── file2.c.c2rust  # 预处理后的文件（或 .i 文件）
        └── selected_files.json     # 用户选择的文件列表
```

### 构建产物追踪 (targets.list)

`c2rust-build` 会自动追踪构建过程中生成的所有二进制文件，并将它们记录在 `targets.list` 文件中。

**文件位置**：`.c2rust/<feature>/c/targets.list`

**包含的二进制类型**：
- **静态库**：本工程目录下的静态库文件（`.a` 文件，以 `lib` 开头）
- **动态库**：构建的共享库文件（`.so` 文件）
- **可执行程序**：构建的可执行二进制文件

**文件格式**：
```
libexample.a
libfoo.so
my_program
another_binary
```

每个二进制文件名占一行，按字母顺序排列。

**用途**：
- 追踪项目构建了哪些二进制产物
- 便于后续处理和 Rust 混合构建
- 帮助确定哪些库需要与转换后的 Rust 代码集成

**注意事项**：
- `targets.list` 在每次构建时自动生成和更新
- 只包含最终的二进制产物，不包含中间文件（如 `.o` 文件）
- 静态库只包含本工程目录下的 `.a` 文件，不包含系统库

## Hook 库工作原理

Hook 库 (`libhook.so`) 使用 LD_PRELOAD 机制拦截编译器调用并生成预处理文件：

1. **拦截机制**：通过 `LD_PRELOAD` 环境变量注入到所有子进程
2. **编译器检测**：拦截 `execve` 系统调用，检测 gcc/clang/cc 调用（支持绝对路径）
3. **预处理文件生成**：**新的 libhook.so 在编译过程中直接生成预处理文件**，无需再调用 `clang -E`
4. **信息记录**：记录编译选项、文件路径和工作目录
5. **输出格式**：使用 `---ENTRY---` 分隔符格式化输出
6. **线程安全**：使用文件锁处理并行构建

**重要变更**：
- 预处理文件会直接生成到 `<C2RUST_PROJECT_ROOT>/.c2rust/<feature>/c/` 目录
- 不再需要在工具里调用 `clang -E` 来处理预处理
- 预处理文件可能是 `.c2rust` 后缀或 `.i` 扩展名

工具使用的环境变量（自动设置）：
- `C2RUST_ROOT`: 项目根目录的绝对路径（用于过滤项目内的文件）
- `LD_PRELOAD`: hook 库的路径（由 c2rust-build 自动设置）

## 配置存储

该工具使用 `c2rust-config` 存储构建配置。这些配置可以稍后由其他 c2rust 工具检索。

### 配置内容

**项目特定配置（保存到项目的 `.c2rust/config.toml`）：**
- `build.dir`: 构建目录（相对于项目根目录）
- `build.cmd`: 完整的构建命令字符串
- 可以关联到特定的特性（通过 `--feature` 参数）

### 配置示例

默认特性的配置：
```
build.dir = "."
build.cmd = "make"
```

使用特性 "debug" 的配置：
```
build.dir = "build"
build.cmd = "make -j4"
```

## 错误处理

工具将在以下情况下退出并显示错误：
- 在 PATH 中找不到 `c2rust-config`
- 未设置 `C2RUST_HOOK_LIB` 环境变量
- Hook 库文件不存在
- 缺少必需的构建命令参数（在 `--` 之后）
- 构建命令执行失败
- 无法保存配置
- 文件选择被取消或失败

## 构建追踪

该工具使用 LD_PRELOAD 钩子库追踪编译器调用并生成预处理文件：

- 创建共享库 (`libhook.so`) 来拦截系统调用
- 通过 `LD_PRELOAD` 注入到构建过程
- 拦截所有编译器调用（gcc/clang/cc），无论是相对路径还是绝对路径
- **新的 libhook.so 在构建过程中直接生成预处理文件**
- 在构建期间记录编译命令和选项
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

注意：如果未安装 `c2rust-config`，一些测试可能会失败。

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
- **Rust**: 1.70 或更高版本（用于构建主程序）
- **新的 libhook.so**: 支持直接生成预处理文件的版本

## 故障排除

### Hook 库未找到

```
Error: Hook library not found. Set C2RUST_HOOK_LIB environment variable to the path of libhook.so
```

**解决方案**：
```bash
export C2RUST_HOOK_LIB=/absolute/path/to/c2rust-build/hook/libhook.so
```

### 未生成预处理文件

如果构建完成后没有生成预处理文件：
1. 确保使用的是支持预处理文件生成的新版本 libhook.so
2. 确保 `C2RUST_HOOK_LIB` 已正确设置
3. 验证 hook 库已编译：`ls -l $C2RUST_HOOK_LIB`
4. 检查构建命令是否实际编译了 C 文件
5. 确保在 Linux 上运行（LD_PRELOAD 要求）

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
