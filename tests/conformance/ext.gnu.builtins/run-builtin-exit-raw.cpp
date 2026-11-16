// __builtin__Exit and __builtin__exit end the program at once: no atexit
// handler runs, so nothing is printed (GCC's builtins of _Exit and _exit;
// clang 22 has neither). Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang has no __builtin__Exit
//@ oracle: gcc
//@ strict-diags
//@ run-args: a
//@ exit: 2
#include <cstdio>
#include <cstdlib>

static void handler() { std::puts("handler"); }

int main(int argc, char **) {
  std::atexit(handler);
  if (argc > 2)
    __builtin__Exit(3);
  __builtin__exit(argc);
}
