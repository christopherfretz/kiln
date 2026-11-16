// [expr.new]/7, [dcl.init.string]: an array new-expression of char type
// may be initialized from a string literal, braced or (C++20, P0960) in
// parentheses. An omitted bound is the literal's length including the
// terminator; a larger bound zero-fills the remaining elements.
//@ expect: run
//@ std: c++20
//@ stdout: foo 4 abc 0 0 hi 3
#include <cstdio>
#include <cstring>

int main() {
  char* a = new char[]{"foo"};
  char* b = new char[6]{"abc"};
  char* c = new char[]("hi");
  std::printf("%s %d %s %d %d %s %d\n", a, (int)sizeof("foo"), b, b[4], b[5],
              c, (int)std::strlen(c) + 1);
  delete[] a;
  delete[] b;
  delete[] c;
}
