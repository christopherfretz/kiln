// GCC's __builtin_exit, __builtin__Exit and __builtin__exit are calls of
// the library exit, _Exit and _exit (noreturn). clang 22 has none of
// them; kiln declares them as it does its other GCC-only builtins, in
// every dialect, while __has_builtin answers clang's 0 (GCC's is 1, so
// the test does not ask). A contract
// violation handler written for GCC calls them (found running GCC's C++
// testsuite). exit runs the atexit handlers and flushes stdout; the run
// argument count picks the builtin.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang has no __builtin_exit
//@ oracle: gcc
//@ strict-diags
//@ exit: 7
//@ stdout: handler
#include <cstdio>
#include <cstdlib>

static void handler() { std::puts("handler"); }

int main(int argc, char **) {
  std::atexit(handler);
  if (argc > 2)
    __builtin__Exit(3);
  if (argc > 1)
    __builtin__exit(2);
  __builtin_exit(7);
}
