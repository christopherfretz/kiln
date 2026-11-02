// [cstring.syn]: `__builtin_strlen` and a later direct call of `strlen` name
// the same C library function, so the program links and both calls agree
// with clang (WP-286: kiln declared a second, undefined `strlen.1`).
//@ expect: run
//@ exit: 5
#include <string.h>
unsigned long a(const char* p) { return __builtin_strlen(p); }
unsigned long b(const char* p) { return strlen(p); }
int main() {
  volatile const char* s = "ab";
  return a((const char*)s) + b("abc");
}
