# sync-helper

一个基于 `rsync` 的交互式 CLI 工具，用来执行精确、可恢复的文件同步。它会在本地保存轻量级服务器配置，让你通过菜单选择目标，然后先生成一份明确的同步计划，再执行 `rsync`。

## 功能特性

- 交互式菜单，支持选择或新增服务器
- 独立的服务器管理菜单，支持编辑和删除
- 支持 `push`（本地 -> 远端）和 `pull`（远端 -> 本地）
- 提供明确的同步策略：`Fast`、`Strict`、`Mirror`
- 支持 `dry-run` 预演
- 可选将本地 `.gitignore` 作为 `rsync` 排除规则使用
- 可选排除 `.git/` 目录
- 配置文件保存在系统标准配置目录
- 支持拖拽路径输入，并自动去除外层引号
- 每个服务器分别记住 `push` 和 `pull` 的默认配置
- 直接调用 `rsync`，并显示变更明细与统计信息

## 环境要求

- Rust（稳定版）
- 系统 `PATH` 中可用的 `rsync`
- 可以通过 SSH 访问远端服务器，推荐使用密钥认证

## 安装

```bash
cargo build --release
```

编译后的二进制位于：

```bash
target/release/shp
```

安装后的命令名是 `shp`，仓库和包名仍然是 `sync-helper`。

## 使用方式

```bash
cargo run
```

或者直接运行构建好的二进制：

```bash
shp
```

首次运行时，程序会提示你创建服务器配置：

- `User`：SSH 用户名
- `Host`：服务器 IP 或域名

程序会保存 `user/host`，并为每台服务器分别记住 `push` 和 `pull` 的默认同步配置。

选择服务器后，就可以直接进入传输流程。程序会在执行前展示一份明确的同步计划，后续再次运行时也会优先复用当前模式下的历史配置。

### Push 路径规则

- 如果远端目标路径以 `/` 结尾，它会被视为“基础目录”，程序会自动追加本地文件或目录名
- 如果远端目标路径不以 `/` 结尾，它会被视为“精确目标路径”
- 如果本地路径是目录，那么本地路径末尾带不带 `/` 在当前实现里没有区别，程序会按同样的目录名解析和同步

示例：

- 本地目录是 `/Users/alice/project`，远端目标填 `/srv/www/`
  最终远端路径会解析为 `/srv/www/project`
- 本地目录是 `/Users/alice/project/`，远端目标填 `/srv/www/`
  最终远端路径仍然会解析为 `/srv/www/project`
- 本地目录是 `/Users/alice/project`，远端目标填 `/srv/www/project`
  最终远端路径就是 `/srv/www/project`
- 本地文件是 `/Users/alice/app.tar.gz`，远端目标填 `/srv/releases/`
  最终远端路径会解析为 `/srv/releases/app.tar.gz`
- 本地文件是 `/Users/alice/app.tar.gz`，远端目标填 `/srv/releases/latest.tar.gz`
  最终远端路径就是 `/srv/releases/latest.tar.gz`

### Pull 路径规则

- 远端源路径按输入原样使用
- 如果远端源路径以 `/` 结尾，`rsync` 会拉取该目录的内容

示例：

- 远端源路径填 `/srv/www/project/`，本地目标目录填 `/Users/alice/workspace`
  会把 `/srv/www/project/` 目录中的内容拉取到 `/Users/alice/workspace`
- 远端源路径填 `/srv/www/project`，本地目标目录填 `/Users/alice/workspace`
  会把远端的 `project` 作为一个整体拉取到 `/Users/alice/workspace/project`
- 远端源路径填 `/srv/releases/latest.tar.gz`，本地目标目录填 `/Users/alice/downloads`
  会把这个文件拉取到 `/Users/alice/downloads/latest.tar.gz`

如果当前模式已经有历史配置，可以一步复用上次参数。

## 同步策略

- `Fast`：按文件大小和修改时间比较，保留目标端多余文件
- `Strict`：按校验和比较，保留目标端多余文件
- `Mirror`：按校验和比较，并删除远端多余文件

`Mirror` 只允许用于 `push`，且本地源必须是目录。

## 过滤规则

过滤模式通过菜单选择：

- `none`：不做额外过滤
- `exclude .git/`：只排除 `.git/`
- `apply local .gitignore as rsync exclude rules`：把本地 `.gitignore` 当作 `rsync` 排除规则
- `both`：同时应用 `.gitignore` 和 `.git/` 排除

关于 `.gitignore` 过滤，当前实现支持的是：

- 读取本地路径附近向上查找到的第一个 `.gitignore` 文件，并把它传给 `rsync --exclude-from`
- 按 `rsync --exclude-from` 的规则文件语义解析，而不是按 Git ignore 语义解析
- 文件按“每行一条规则”处理；空行会被忽略，整行以 `#` 或 `;` 开头的注释也会被忽略
- 没有前缀的行会被当作排除规则；以 `- ` 开头的行会被当作排除规则，以 `+ ` 开头的行会被当作包含规则
- 如果某一行只有 `!`，会清空当前已经累积的过滤规则
- 支持常见的 `rsync` 匹配语义：
  - `target/`：只匹配目录
  - `*.log`：`*` 匹配任意非 `/` 字符
  - `file?.txt`：`?` 匹配单个非 `/` 字符
  - `[ab].txt`：字符类匹配
  - `foo/bar`：包含 `/` 时按传输路径中的完整相对路径匹配
  - `/foo`：以 `/` 开头时，锚定到本次传输根路径
  - `foo/**/bar` 或 `foo/***`：支持 `**` 跨目录匹配，`***` 可匹配目录及其全部内容
- 适合常见的排除场景，比如忽略 `target/`、`node_modules/`、`.DS_Store`、`*.log` 这类模式
- 可以和 `exclude .git/` 组合使用

当前实现不保证支持的是：

- 与 Git 完全一致的忽略语义
- 多层目录中的多个 `.gitignore` 逐层叠加
- `.git/info/exclude` 和全局 gitignore
- 所有复杂的反向包含、锚定路径、目录边界规则都与 Git 行为完全一致

如果你需要“结果必须和 Git 一模一样”的过滤行为，这个项目当前版本不应被视为完整支持。

开始执行前，程序会显示传输摘要，并要求你做最后确认。

## 实际执行的命令形态

```bash
# push 模式
rsync -azP --itemize-changes --stats -e "ssh -p <port>" [策略参数] [过滤参数] <local_source> <user>@<host>:<remote_target>

# pull 模式
rsync -azP --itemize-changes --stats -e "ssh -p <port>" [策略参数] [过滤参数] <user>@<host>:<remote_source> <local_destination>
```

## 配置文件位置

配置文件保存在系统标准配置目录：

- Windows: `%APPDATA%\sync-helper\config.toml`
- macOS: `~/Library/Application Support/sync-helper/config.toml`
- Linux: `~/.config/sync-helper/config.toml`

## 说明

- 如果 `config.toml` 损坏，程序会询问是否重置
- 传输失败时，会直接显示 `rsync` 的错误输出
- `.gitignore` 过滤实际上是通过 `rsync --exclude-from` 实现的，所以使用的是 `rsync` 的排除语义，不是完整的 Git ignore 语义
- 启用 `.git/` 排除时，会传入 `rsync --exclude=.git/`
- 在 `push` 模式下，程序会先执行 `ssh "mkdir -p ..."`，确保远端目标目录或父目录已经存在

## License

MIT
