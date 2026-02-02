# 发布指南

本文档介绍如何将 `c2rust-build` 发布到 crates.io。

## 前置条件

### 1. 配置 GitHub Secret

在发布之前，仓库维护者需要在 GitHub 仓库设置中添加 crates.io API token：

1. 进入 GitHub 仓库页面
2. 点击 **Settings** (设置)
3. 在左侧菜单中选择 **Secrets and variables** -> **Actions**
4. 点击 **New repository secret**
5. 添加以下 secret：
   - **Name**: `CARGO_REGISTRY_TOKEN`
   - **Value**: `ciogp5lbOh98UgYZMGzdiID3Qf70KWkutyc`

### 2. 确保代码质量

在发布之前，确保：
- 所有测试通过：`cargo test`
- 代码能够成功构建：`cargo build --release`
- 文档是最新的

## 发布流程

### 自动发布（推荐）

本项目配置了 GitHub Actions 自动发布工作流。当推送符合 `v*` 格式的 git tag 时，会自动触发发布流程。

**步骤：**

1. 确保 `Cargo.toml` 中的版本号已更新
2. 提交所有更改：
   ```bash
   git add .
   git commit -m "Prepare for release v0.1.0"
   ```

3. 创建并推送 tag：
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. GitHub Actions 将自动：
   - 检出代码
   - 安装 Rust 工具链
   - 运行测试
   - 发布到 crates.io

5. 在 GitHub 的 **Actions** 标签页中查看发布进度

### 手动发布

如果需要手动发布，可以使用以下命令：

1. **测试发布（不会实际发布）**：
   ```bash
   cargo publish --dry-run
   ```

2. **实际发布**：
   ```bash
   cargo publish --token ciogp5lbOh98UgYZMGzdiID3Qf70KWkutyc
   ```

## 版本号规范

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/) 规范：

- **主版本号（MAJOR）**：当做了不兼容的 API 修改
- **次版本号（MINOR）**：当做了向下兼容的功能性新增
- **修订号（PATCH）**：当做了向下兼容的问题修正

### 版本号示例

- `0.1.0` - 初始版本
- `0.1.1` - 修复 bug
- `0.2.0` - 添加新功能（向下兼容）
- `1.0.0` - 稳定版本发布
- `2.0.0` - 重大更新（不兼容的 API 变更）

### Tag 格式

创建 tag 时，请使用 `v` 前缀：

```bash
git tag v0.1.0      # ✓ 正确
git tag 0.1.0       # ✗ 错误（不会触发自动发布）
```

## 发布前检查清单

在发布新版本之前，请确认以下事项：

- [ ] 更新 `Cargo.toml` 中的版本号
- [ ] 更新 `README.md` 中的版本号引用（如有）
- [ ] 更新 `CHANGELOG.md`（如有）
- [ ] 确保所有测试通过：`cargo test`
- [ ] 确保代码能够构建：`cargo build --release`
- [ ] 运行 `cargo publish --dry-run` 测试发布流程
- [ ] 检查 `Cargo.toml` 中的元数据是否完整
- [ ] 提交所有更改并推送到 GitHub
- [ ] 创建并推送相应的版本 tag

## 发布后操作

发布成功后：

1. 在 [crates.io](https://crates.io/crates/c2rust-build) 上验证新版本是否可用
2. 在 GitHub 上创建 Release，说明更新内容
3. 更新相关文档和示例

## 回滚

**重要提示**：发布到 crates.io 是**不可逆**操作。一旦发布，无法删除或修改已发布的版本。

如果发布了有问题的版本：

1. 修复问题
2. 发布新的修订版本（例如从 `0.1.0` 到 `0.1.1`）
3. 使用 `cargo yank` 标记有问题的版本为已撤回（不推荐使用）：
   ```bash
   cargo yank --vers 0.1.0 --token ciogp5lbOh98UgYZMGzdiID3Qf70KWkutyc
   ```

## 故障排除

### 发布失败

如果 GitHub Actions 发布失败：

1. 检查 Actions 日志以了解错误详情
2. 常见问题：
   - `CARGO_REGISTRY_TOKEN` 未配置或已过期
   - 版本号已存在于 crates.io
   - 测试失败
   - 缺少必需的元数据字段

### Token 过期

如果 crates.io API token 过期：

1. 登录 [crates.io](https://crates.io/)
2. 进入 **Account Settings** -> **API Tokens**
3. 生成新的 token
4. 更新 GitHub Secret `CARGO_REGISTRY_TOKEN`

## 相关资源

- [crates.io 发布指南](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [语义化版本规范](https://semver.org/lang/zh-CN/)
- [Cargo.toml 清单格式](https://doc.rust-lang.org/cargo/reference/manifest.html)
