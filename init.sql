-- MySQL 初始化脚本
-- 为 crabot 项目设置数据库用户和权限

USE mysql;

-- 确保用户存在且设置正确的权限
CREATE USER IF NOT EXISTS 'crabot'@'localhost' IDENTIFIED BY 'crabot_password';
CREATE USER IF NOT EXISTS 'crabot'@'%' IDENTIFIED BY 'crabot_password';

-- 授予所有权限
GRANT ALL PRIVILEGES ON crabot.* TO 'crabot'@'localhost';
GRANT ALL PRIVILEGES ON crabot.* TO 'crabot'@'%';

-- 刷新权限
FLUSH PRIVILEGES;

-- 使用 crabot 数据库
USE crabot;

-- 设置初始数据（可选）
SELECT 'crabot 数据库已初始化' as status;
