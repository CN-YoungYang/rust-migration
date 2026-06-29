# 1Panel 部署指南

## 前置准备

1. **生成加密密钥**
```bash
openssl rand -base64 32
```
保存输出的密钥，后面需要用到。

## 1Panel 部署步骤

### 方式 1: 使用 Docker Compose (推荐)

#### 1. 在 1Panel 中创建应用

1. 进入 1Panel → **应用商店** → **Docker Compose**
2. 点击 **创建应用**
3. 应用名称: `ai-hub-rust`
4. 工作目录: `/opt/1panel/apps/ai-hub-rust`

#### 2. 上传项目文件

使用 1Panel 文件管理器或 SSH 上传以下文件到工作目录:
```
/opt/1panel/apps/ai-hub-rust/
├── docker-compose.hub.yml
├── Dockerfile
├── Cargo.toml
├── Cargo.lock
├── .env
├── src/
├── frontend/
├── public/
└── migrations/
```

#### 3. 配置 docker-compose.hub.yml

推荐直接使用项目内 `docker-compose.hub.yml`。如需在 1Panel 中手动编辑，保持主机端口 `3000` 映射到容器端口 `8080`:

```yaml
version: ''3.8''

services:
  app:
    build: .
    container_name: ai-hub-rust
    ports:
      - "3000:8080"
    env_file:
      - .env
    environment:
      - DATABASE_URL=sqlite:/app/data/ai-hub.db
      - RUST_LOG=warn
    volumes:
      - ./data:/app/data
    restart: unless-stopped
    mem_limit: 200m
    mem_reservation: 100m
    cpus: 0.9
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

#### 4. 启动应用

1. 在 1Panel Docker Compose 界面
2. 找到 `ai-hub-rust` 应用
3. 点击 **构建** (首次需要，约 15-30 分钟)
4. 构建完成后点击 **启动**

#### 5. 配置反向代理 (可选)

1. 进入 1Panel → **网站** → **创建网站**
2. 选择 **反向代理**
3. 配置:
   - 域名: `your-domain.com`
   - 代理地址: `http://127.0.0.1:3000`
4. 保存并启用 SSL

### 方式 2: 预编译二进制 (更快)

如果 1C1G 服务器编译太慢，可以本地编译后上传:

#### 1. 本地编译
```bash
# 本地或其他服务器
cd rust-migration
cargo build --release

# 打包二进制和依赖
tar -czf ai-hub-rust.tar.gz \
  target/release/ai-hub-rust \
  migrations/ \
  .env.example
```

#### 2. 上传到服务器
```bash
scp ai-hub-rust.tar.gz user@your-server:/opt/1panel/apps/
```

#### 3. 在 1Panel 中解压运行
```bash
cd /opt/1panel/apps/
tar -xzf ai-hub-rust.tar.gz
cp .env.example .env
vim .env  # 配置环境变量

# 运行
./target/release/ai-hub-rust
```

#### 4. 配置为系统服务

在 1Panel → **工具箱** → **Supervisor** 中添加:
```ini
[program:ai-hub-rust]
command=/opt/1panel/apps/ai-hub-rust/target/release/ai-hub-rust
directory=/opt/1panel/apps/ai-hub-rust
autostart=true
autorestart=true
user=root
stdout_logfile=/opt/1panel/apps/ai-hub-rust/logs/stdout.log
stderr_logfile=/opt/1panel/apps/ai-hub-rust/logs/stderr.log
environment=RUST_LOG="warn",DATABASE_URL="sqlite:./data/ai-hub.db",TOKEN_ENCRYPTION_KEY="你的密钥"
```

## 1Panel 特定配置

### 1. 内存限制

在 1Panel Docker 容器设置中:
- 内存限制: 200MB
- 内存预留: 100MB
- CPU 限制: 0.9

### 2. 端口映射

- 容器端口: 8080
- 主机端口: 3000 (或其他可用端口)

### 3. 环境变量 (在 1Panel 界面配置)

| 变量名 | 值 | 说明 |
|--------|-----|------|
| DATABASE_URL | sqlite:/app/data/ai-hub.db | 数据库路径 |
| TOKEN_ENCRYPTION_KEY | (你的密钥) | **必填** |
| RUST_LOG | warn | 日志级别 |
| ADMIN_USERNAME | admin | 管理员用户名 |
| ADMIN_PASSWORD | 强密码 | 首次启动创建管理员时必填 |
| COOKIE_SECURE | true/false | HTTPS 部署建议 true；HTTP 测试保持 false |

### 4. 数据持久化

在 1Panel 卷映射中:
- 容器路径: `/app/data`
- 主机路径: `./data` (相对于应用目录)

## 常见问题

### Q: 1Panel 中如何查看日志?
A: 
1. 进入 **容器** → 找到 `ai-hub-rust`
2. 点击 **日志** 按钮
3. 或在终端: `docker logs -f ai-hub-rust`

### Q: 构建太慢怎么办?
A: 
1. 使用预编译二进制方式(方式 2)
2. 或在本地/其他服务器构建后导入镜像:
   ```bash
   docker save ai-hub-rust:latest | gzip > ai-hub-rust.tar.gz
   # 上传到服务器
   docker load < ai-hub-rust.tar.gz
   ```

### Q: 如何备份数据?
A: 在 1Panel 文件管理器中备份:
- `/opt/1panel/apps/ai-hub-rust/data/ai-hub.db`

### Q: 如何更新应用?
A:
1. 停止容器
2. 上传新的代码文件
3. 重新构建: `docker compose -f docker-compose.hub.yml build --no-cache`
4. 启动容器

### Q: 内存不足?
A: 
1Panel → **主机** → **监控**
- 如果内存使用 >80%，考虑:
  - 降低 `mem_limit` 到 150m
  - 添加 swap (1Panel 终端):
    ```bash
    sudo fallocate -l 2G /swapfile
    sudo chmod 600 /swapfile
    sudo mkswap /swapfile
    sudo swapon /swapfile
    ```

## 1Panel 反向代理配置示例

### Nginx 配置

```nginx
server {
    listen 80;
    server_name your-domain.com;
    
    # 自动重定向到 HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name your-domain.com;
    
    # SSL 证书(1Panel 自动配置)
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket 支持(如需要)
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

## 快速检查清单

- [ ] 生成 TOKEN_ENCRYPTION_KEY
- [ ] 上传项目文件到 1Panel
- [ ] 配置 docker-compose.hub.yml
- [ ] 设置环境变量
- [ ] 构建并启动容器
- [ ] 访问 http://your-ip:3000/api/health
- [ ] 使用 `.env` 中的管理员账号登录
- [ ] 修改管理员密码
- [ ] 配置反向代理(可选)
- [ ] 配置 SSL(可选)

## 访问测试

```bash
# 健康检查
curl http://your-ip:3000/api/health

# 登录测试会设置 Cookie；浏览器验证更直观
curl -i -X POST http://your-ip:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d ''{"username":"admin","password":"你的管理员密码"}''
```

## 1Panel 监控

在 1Panel 仪表板可以看到:
- CPU 使用率: 应该 <5%
- 内存使用: 应该 ~50-100MB
- 网络流量: 根据访问量

---

**总结**: 1Panel 部署非常简单，图形界面操作，无需命令行! 🎉
