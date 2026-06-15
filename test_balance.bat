@echo off
echo ======================================
echo 余额查询功能测试
echo ======================================
echo.

cd /d "%~dp0"

echo [1/3] 检查 Rust 环境...
cargo --version >nul 2>&1
if errorlevel 1 (
    echo ERROR: Cargo 未安装或不在 PATH 中
    pause
    exit /b 1
)
echo OK: Cargo 已安装
echo.

echo [2/3] 编译项目（Release 模式）...
echo 这可能需要几分钟，请耐心等待...
echo.
cargo build --release
if errorlevel 1 (
    echo ERROR: 编译失败，请检查错误信息
    pause
    exit /b 1
)
echo OK: 编译成功
echo.

echo [3/3] 启动服务...
echo 日志将显示在下方，按 Ctrl+C 停止服务
echo.
echo ======================================
echo 服务日志输出：
echo ======================================
echo.

target\release\ai-hub-rust.exe

pause
