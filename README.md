# Pingora Proxy Simple

基于 [Pingora](https://github.com/cloudflare/pingora) 的轻量级反向代理服务器，用于替代 nginx 托管静态文件和 API 代理。

## 功能

- 🚀 **多端口监听** — 一个进程监听多个端口
- 📁 **静态文件托管** — 自动 MIME 类型检测，支持 SPA fallback
- 🔄 **API 反向代理** — 将指定路径代理到后端服务
- ⚙️ **YAML 配置** — 简单易懂的声明式配置

## 快速开始

### 构建

```bash
cargo build --release
```

### 配置

编辑 `config.yaml`：

```yaml
servers:
  # 静态文件 + API 同端口示例
  - listen: "0.0.0.0:7000"
    name: "my-app"
    locations:
      - path: "/api"           # API 代理
        upstream: "127.0.0.1:3001"
      - path: "/"              # 静态文件
        static_root: "/var/www/my-app"

  # 纯静态文件示例
  - listen: "0.0.0.0:80"
    name: "default"
    locations:
      - path: "/"
        static_root: "/var/www/html"

  # 纯 API 代理示例
  - listen: "0.0.0.0:7002"
    name: "api-server"
    locations:
      - path: "/"
        upstream: "127.0.0.1:4000"
```

### 运行

```bash
./target/release/pingora-proxy
```

## 配置说明

| 字段 | 说明 |
|------|------|
| `listen` | 监听地址，格式 `IP:PORT` |
| `name` | 服务名称（日志用） |
| `locations[].path` | URL 路径前缀 |
| `locations[].upstream` | 后端服务地址（可选） |
| `locations[].static_root` | 静态文件目录（可选） |

**匹配规则：**
- 路径按长度优先匹配（`/api` 优先于 `/`）
- 有 `upstream` 的走代理
- 无 `upstream` 有 `static_root` 的走静态文件
- 静态文件找不到时，返回 `index.html`（支持 SPA）

## systemd 服务

```bash
# 复制服务文件
sudo cp pingora-proxy.service /etc/systemd/system/

# 启动服务
sudo systemctl daemon-reload
sudo systemctl enable pingora-proxy
sudo systemctl start pingora-proxy
```

## 对比 nginx

| 特性 | nginx | pingora-proxy |
|------|-------|---------------|
| 配置复杂度 | 高 | 低 |
| 内存占用 | ~10MB | ~5MB |
| 热更新 | 需 reload | 需重启 |
| 扩展性 | Lua/模块 | Rust 代码 |

适合简单场景：个人项目、小型服务、开发环境。

## 依赖

- Rust 1.70+
- Pingora 0.8

## 说明

本项目代码由 AI（Claude）生成，人类仅提供需求和测试。🤖

## License

MIT
