// C23 6.2.2 and C++ [dcl.link]/1-4: a C translation unit calls a C++
// function declared `extern "C"` in a C++ unit, linked into one program.
//@ expect: run
//@ unit: Inputs/cxx-impl.cpp
//@ stdout: 42 c++
#include <stdio.h>
int cxx_twice(int);
const char *cxx_lang(void);
int main(void) {
  printf("%d %s\n", cxx_twice(21), cxx_lang());
  return 0;
}
