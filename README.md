# depctl

一个用 Rust 实现的依赖管理工具，是对 [depsync](https://github.com/domchen/depsync) 的 Rust 重写实现，但不需要 Node.js 运行时。

## 关于

depctl 是对 [domchen/depsync](https://github.com/domchen/depsync) 的 Rust 实现版本。depsync 是一个用 Node.js 实现的依赖管理工具，通过 DEPS 配置文件自动同步项目依赖。depctl 提供了相同的功能和兼容的 DEPS 文件格式，但作为独立的二进制文件运行，无需 Node.js 运行时。

## 功能特性

depctl 提供了自动同步项目依赖的便捷方式，通过 DEPS 配置文件来管理依赖：

- **浅克隆仓库**：始终使用 depth=1 克隆 Git 仓库，保持项目体积相对紧凑
- **自动处理子模块和 LFS**：自动为主项目和第三方仓库下载 git-submodules 和 git-lfs 文件
- **LFS 缓存优化**：下载过程中可以跳过本地缓存中已存在的 git-lfs 文件，即使对于浅克隆仓库也有效
- **自定义动作**：同步后可以执行自定义命令，例如清理任务等
- **平台特定依赖**：可以为每个平台单独配置仓库和文件，避免下载不必要的依赖
- **URL 镜像支持**：支持将仓库和文件 URL 重定向到镜像源
- **递归处理**：默认递归处理所有子仓库中的 DEPS 文件

## 安装

### 从源码编译

```bash
git clone <repository-url>
cd depctl
cargo build --release
```

编译后的二进制文件位于 `target/release/depctl`。

### 使用 Cargo 安装

```bash
cargo install --path .
```

### 使用 Homebrew 安装

```bash
brew install 0x1306a94/tap/depctl --verbose
```

## 使用方法

在包含 DEPS 文件的目录中运行：

```bash
depctl [platform] [options]
```

### 基本用法

例如，如果要同步 mac 平台，运行：

```bash
depctl mac
```

如果不传递任何平台参数，将自动选择主机平台作为目标平台。因此在 macOS 上运行 `depctl` 等同于运行 `depctl mac`。

可用的平台名称在 DEPS 文件中定义，您也可以定义任何其他平台名称，如 `ios`、`android` 等，但只有 `mac`、`win` 和 `linux` 可以自动选择。

### 命令行选项

#### `-h, --help`
打印帮助信息。

```bash
depctl --help
```

#### `-v, --version`
打印当前版本。

```bash
depctl --version
```

#### `-p, --project <directory>`
同步指定目录中的项目，而不是当前目录。

```bash
depctl --project /path/to/your/project
depctl mac --project /usr/local/myproject
```

#### `-c, --clean`
清理当前目录中不存在于 DEPS 文件中的仓库和文件。这有助于删除过时的依赖。

```bash
depctl --clean
```

#### `--non-recursive`
跳过同步子项目。默认情况下，depctl 会递归处理所有子仓库中的 DEPS 文件。

```bash
depctl --non-recursive
depctl mac --non-recursive
```

#### `--force-linkfiles`
强制重新创建 linkfiles，即使目标已存在。默认情况下，如果目标已存在，linkfiles 会被跳过。

```bash
depctl --force-linkfiles
```

#### `--force-copyfiles`
强制重新创建 copyfiles，即使目标已存在。默认情况下，如果目标已存在，copyfiles 会被跳过。

```bash
depctl --force-copyfiles
```

#### `--mirror <mappings>`
将仓库和文件 URL 重定向到镜像源。这在以下情况下特别有用：
- 使用镜像仓库而不是原始仓库
- 重定向到内部企业仓库
- 替换会递归应用到所有子仓库

格式：单个映射为 `'old_prefix->new_prefix'`，多个映射为 `'old1->new1,old2->new2'`。

```bash
# 单个镜像
depctl --mirror 'https://github.com/libpag/->https://gitee.com/pago/'

# 多个镜像（逗号分隔）
depctl --mirror 'https://github.com/->https://gitee.com/,https://gitlab.com/->https://internal.company.com/'
```

镜像选项会替换整个依赖树中的仓库 URL 和文件下载 URL 的前缀。

### DEPS 文件格式

以下是 DEPS 文件的示例：

```json
{
  "version": "1.4.5",
  "vars": {
    "GIT_DOMAIN": "github.com",
    "SKIA_ROOT": "https://github.com/domchen/depsync/releases/download/1.0.1",
    "V8_ROOT": "https://github.com/domchen/depsync/releases/download/1.0.2"
  },
  "repos": {
    "common": [
      {
        "url": "https://${GIT_DOMAIN}/webmproject/libwebp.git",
        "commit": "1fe3162541ab2f5ce69aca2e2b593fab60520d34",
        "dir": "third_party/libwebp"
      }
    ]
  },
  "files": {
    "common": [
      {
        "url": "${SKIA_ROOT}/include.zip",
        "dir": "third_party/skia",
        "unzip": true
      }
    ],
    "mac": [
      {
        "url": "${SKIA_ROOT}/darwin-x64.zip",
        "dir": "third_party/skia",
        "unzip": true
      }
    ]
  },
  "actions": {
    "common": [
      {
        "command": "depctl --clean",
        "dir": "third_party"
      }
    ]
  },
  "linkfiles": {
    "common": [
      {
        "src": "third_party/skia/include/skia",
        "dest": "include/skia"
      },
      {
        "src": "third_party/v8/include/v8",
        "dest": "include/v8"
      }
    ]
  },
  "copyfiles": {
    "common": [
      {
        "src": "third_party/skia/LICENSE",
        "dest": "licenses/skia/LICENSE"
      }
    ]
  }
}
```

### linkfiles 和 copyfiles 配置

`linkfiles` 和 `copyfiles` 用于在依赖同步完成后自动创建软链接或复制文件，类似于 AOSP manifest.xml 中的功能。

#### linkfiles

在依赖同步后创建软链接。格式：

```json
"linkfiles": {
  "common": [
    {
      "src": "third_party/skia/include/skia",
      "dest": "include/skia"
    }
  ],
  "mac": [
    {
      "src": "third_party/skia/mac/lib/libskia.dylib",
      "dest": "lib/libskia.dylib"
    }
  ]
}
```

- `src`: 源文件或目录路径（相对于项目根目录）
- `dest`: 目标软链接路径（相对于项目根目录）
- 支持平台特定配置（`common`, `mac`, `win`, `linux` 等）
- 支持变量替换（`${VAR}`）
- **默认行为**：如果目标已存在，会跳过创建（不会覆盖）
- **强制模式**：使用 `--force-linkfiles` 参数可以强制重新创建，即使目标已存在

#### copyfiles

在依赖同步后复制文件或目录。格式：

```json
"copyfiles": {
  "common": [
    {
      "src": "third_party/skia/LICENSE",
      "dest": "licenses/skia/LICENSE"
    },
    {
      "src": "third_party/v8/docs",
      "dest": "docs/v8"
    }
  ]
}
```

- `src`: 源文件或目录路径（相对于项目根目录）
- `dest`: 目标路径（相对于项目根目录）
- 支持递归复制目录
- 支持平台特定配置和变量替换
- **默认行为**：如果目标已存在，会跳过复制（不会覆盖）
- **强制模式**：使用 `--force-copyfiles` 参数可以强制重新复制，即使目标已存在

**执行顺序**：
1. 同步所有 repos（Git 仓库，包括递归的子项目）
2. 处理每个 repo 的 git submodules 和 LFS
3. 下载和解压所有 files（文件）
4. 处理主项目的 git submodules 和 LFS（确保所有文件都已下载）
5. **栈式执行 linkfiles 和 copyfiles**：
   - 在同步过程中，遇到 linkfiles/copyfiles 时**入栈**（不立即执行）
   - 在所有依赖（包括递归的子项目）都同步完成后，**出栈依次执行**
   - 这样可以确保即使依赖链 A -> B -> C -> D，A 的 linkfiles/copyfiles 的源是 C 或 D，也能正确执行
6. 执行 actions（自定义命令）

**为什么使用栈式执行？**

在依赖链 A -> B -> C -> D 的情况下，如果 A 的 linkfiles/copyfiles 的源是 C 或 D：
- 如果立即执行：A 的 linkfiles/copyfiles 会在 C 和 D 同步完成前执行，导致源文件不存在
- 使用栈式执行：所有 linkfiles/copyfiles 在所有同步完成后才执行，确保源文件一定存在

## 与 depsync 的兼容性

depctl 是对 [domchen/depsync](https://github.com/domchen/depsync) 的 Rust 实现，完全兼容 depsync 的 DEPS 文件格式和命令行接口。主要区别：

1. **不需要 Node.js**：depctl 是独立的二进制文件，不需要 Node.js 运行时
2. **性能更好**：Rust 实现提供了更好的性能
3. **单文件分发**：编译后的二进制文件可以独立分发，无需安装依赖
4. **完全兼容**：支持所有 depsync 的功能，包括 actions 中的 `depsync` 命令会自动替换为 `depctl`

### 参考实现

- 原始项目：[domchen/depsync](https://github.com/domchen/depsync) (Node.js 实现)
- 本实现：depctl (Rust 实现)

## 环境变量

- `GIT_USER`：Git 仓库认证用户名
- `GIT_PASSWORD`：Git 仓库认证密码
- `DomainName`：格式为 `username:password@domain` 的认证信息（备用方式）

## 许可证

MIT License
