@echo off
chcp 65001 >nul
echo ======================================
echo Git 提交工具
echo ======================================
echo.

cd /d "%~dp0"

echo [1/4] 检查修改的文件...
git status --short
echo.

echo [2/4] 添加所有修改的文件...
git add src/services/checkin/providers/new_api.rs
git add src/services/checkin/providers/anyrouter.rs
git add src/services/checkin/providers/x666.rs
git add src/routes/accounts.rs
git add BALANCE_FIX.md
git add test_balance.bat
git add test_balance.md
git add COMMIT_MESSAGE.md
echo 完成
echo.

echo [3/4] 查看待提交内容...
git status
echo.

echo ======================================
echo 请选择提交信息格式：
echo ======================================
echo 1. 简短版（推荐）
echo 2. 完整版（详细说明）
echo 3. 中文版
echo 4. 自定义
echo 0. 取消提交
echo.
set /p choice="请输入选择 (0-4): "

if "%choice%"=="0" (
    echo 已取消提交
    pause
    exit /b 0
)

if "%choice%"=="1" (
    echo.
    echo [4/4] 使用简短版提交...
    git commit -m "fix(balance): 增强余额查询容错能力和错误日志" -m "- 支持多种余额字段格式（quota/balance/credit/amount）" -m "- 支持嵌套对象字段（data.quota/data.current_quota）" -m "- 添加详细错误日志（HTTP状态、响应体、解密失败）" -m "- 完全兼容 Next.js 版本逻辑"
    goto :done
)

if "%choice%"=="2" (
    echo.
    echo [4/4] 使用完整版提交...
    git commit -F COMMIT_MESSAGE.md
    goto :done
)

if "%choice%"=="3" (
    echo.
    echo [4/4] 使用中文版提交...
    git commit -m "修复(余额查询): 增强容错能力，支持多种字段格式" -m "" -m "问题：" -m "- 某些站点返回的余额字段名不是 quota，导致查询失败" -m "- 解密失败时错误消息为空，难以排查" -m "" -m "修复：" -m "- 支持 quota/balance/credit/amount 等多种字段" -m "- 支持嵌套结构 data.quota, data.current_quota" -m "- 增加详细错误日志，包含响应体和具体失败原因" -m "- 解密失败单独处理，返回明确提示"
    goto :done
)

if "%choice%"=="4" (
    echo.
    echo 请手动编辑提交信息...
    git commit
    goto :done
)

echo 无效选择，已取消提交
pause
exit /b 1

:done
if errorlevel 1 (
    echo.
    echo 提交失败！请检查错误信息
    pause
    exit /b 1
)

echo.
echo ======================================
echo ✅ 提交成功！
echo ======================================
echo.
echo 最新提交：
git log -1 --oneline
echo.
echo 提交详情：
git log -1 --stat
echo.

echo ======================================
echo 下一步操作：
echo ======================================
echo 1. 推送到远程仓库：git push
echo 2. 查看提交历史：git log
echo 3. 创建标签：git tag v2.2.1
echo.

pause
