#!/bin/bash
# 一键录制 ZMark 运行时 CPU 与 120fps 渲染耗时（Instruments Time Profiler）
# 用法：./scripts/profile_fps.sh [持续秒数，默认 8 秒]
set -e

DURATION="${1:-8}"
APP_PATH="/Applications/ZMark.app"
TRACE_OUTPUT="./target/zmark_profile.trace"

if [ ! -d "$APP_PATH" ]; then
    echo "未找到 $APP_PATH，请先运行 ./scripts/bundle_macos.sh --install 进行打包安装"
    exit 1
fi

rm -rf "$TRACE_OUTPUT"
echo "==> 启动 Instruments Time Profiler 性能录制（持续 $DURATION 秒）..."
echo "==> 请在 ZMark 窗口内进行快速滑动、切换模式或打开大文档..."

xcrun xctrace record \
    --template 'Time Profiler' \
    --time-limit "${DURATION}s" \
    --output "$TRACE_OUTPUT" \
    --launch -- "$APP_PATH/Contents/MacOS/ZMark"

echo "==> 性能录制完成！产物位于: $TRACE_OUTPUT"
echo "==> 正在使用 Instruments 打开图形化分析看板..."
open "$TRACE_OUTPUT"
