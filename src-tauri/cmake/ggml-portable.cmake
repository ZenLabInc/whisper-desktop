# CMAKE_PROJECT_INCLUDE で whisper.cpp の project() 直後に読み込まれる。
# ここで GGML_NATIVE を強制 OFF にし、ビルドマシン固有の CPU 命令
# (Apple Silicon の i8mm 等) を埋め込まない「配布先のどの CPU でも動く」
# 移植性のあるバイナリにする。
#
# whisper.cpp の WHISPER_NATIVE→GGML_NATIVE 変換マクロは ON 固定で OFF に
# できないため、cmake オプションを直接強制する必要がある。
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
