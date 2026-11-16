// spec/driver.md "Link options" (WP-1485): at a plain -std=c++26 kiln++
// links libstdc++exp, which holds GCC 16.1's <stacktrace>, so
// std::stacktrace::current() links and runs without -lstdc++exp (g++
// 16.1 needs the library named).
//@ expect: run
//@ args: --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): the test needs GCC 16.1's libstdc++
//@ stdout: ok
#include <cstdio>
#include <stacktrace>
int main() {
  auto st = std::stacktrace::current();
  std::puts(st.size() <= st.max_size() ? "ok" : "bad");
}
