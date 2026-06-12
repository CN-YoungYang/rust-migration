#!/bin/bash
# deploy-1c1g.sh - 甲骨文 1C1G 一键部署脚本

set -e

echo "=== 甲骨文 1C1G 部署脚本 ==="
echo ""

# 1. 检查 swap
echo "[1/6] 检查 swap..."
if [ ! -f /swapfile ]; then
    echo "正在创建 2GB swap..."
    sudo fallocate -l 2G /swapfile
    sudo chmod 600 /swapfile
    sudo mkswap /swapfile
    sudo swapon /swapfile
    echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
    echo "Swap 创建完成!"
else
    echo "Swap 已存在"
fi

# 2. 调整 swappiness
echo ""
echo "[2/6] 调整 swappiness..."
sudo sysctl vm.swappiness=10
if ! grep -q "vm.swappiness=10" /etc/sysctl.conf; then
    echo 'vm.swappiness=10' | sudo tee -a /etc/sysctl.conf
fi

# 3. 检查 Docker
echo ""
echo "[3/6] 检查 Docker..."
if ! command -v docker &> /dev/null; then
    echo "Docker 未安装,请先安装 Docker"
    exit 1
fi

# 4. 配置环境变量
echo ""
echo "[4/6] 配置环境变量..."
if [ ! -f .env ]; then
    cp .env.example .env
    echo ""
    echo "⚠️  请编辑 .env 文件设置:"
    echo "  1. TOKEN_ENCRYPTION_KEY (运行: openssl rand -base64 32)"
    echo "  2. ADMIN_PASSWORD (可选)"
    echo ""
    read -p "按 Enter 继续..."
fi

# 5. 构建镜像
echo ""
echo "[5/6] 构建 Docker 镜像 (预计 15-30 分钟)..."
echo "⚠️  1C1G 服务器构建较慢,请耐心等待..."
docker compose build --no-cache 2>/dev/null || docker-compose build --no-cache

# 6. 启动服务
echo ""
echo "[6/6] 启动服务..."
docker compose up -d 2>/dev/null || docker-compose up -d

echo ""
echo "=== 部署完成! ==="
echo ""
echo "服务地址: http://$(hostname -I | awk '{print $1}'):3000"
echo "默认管理员: admin / admin123"
echo ""
echo "查看日志: docker compose logs -f"
echo "监控资源: docker stats"
echo ""
echo "⚠️  请立即修改管理员密码!"
