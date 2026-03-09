#!/bin/bash
# crabot 项目启动脚本

set -e

echo "========== OpenClaw 组织管理系统 - 启动脚本 =========="
echo ""

# 检查 MySQL 连接
check_mysql() {
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if mysql -h 127.0.0.1 -u root -proot_password -e "SELECT 1" > /dev/null 2>&1; then
            echo "✅ MySQL 连接成功"
            return 0
        fi
        attempt=$((attempt + 1))
        echo "⏳ 等待 MySQL (尝试 $attempt/$max_attempts)..."
        sleep 1
    done
    
    return 1
}

# 检查 Docker 是否运行
if ! command -v docker &> /dev/null; then
    echo "❌ Docker 未安装，请先安装 Docker"
    exit 1
fi

# 启动 MySQL
echo "📦 启动 MySQL 容器..."
docker-compose up -d mysql

# 等待 MySQL 启动
if check_mysql; then
    echo "✅ MySQL 已准备就绪"
else
    echo "❌ MySQL 启动失败或超时"
    exit 1
fi

# 设置环境变量
export DATABASE_URL="mysql://crabot:crabot_password@127.0.0.1:3306/crabot"

# 构建和运行项目
echo ""
echo "🔨 构建 Rust 项目..."
cargo build --release

echo ""
echo "🚀 运行应用..."
cargo run --release

echo ""
echo "========== 应用运行完成 =========="
