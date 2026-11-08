// [namespace.udecl]/11 with the clang extension pass_object_size: a function with a
// pass_object_size parameter overloads one without (spec/sema-overload.md §4.3a), so a
// using-declaration naming both (glibc's <stdlib.h> fortify wrappers re-exported by
// libstdc++'s <stdlib.h> as `using std::mbstowcs;`) conflicts with neither, and a call picks
// the pass_object_size overload.
//@ expect: run
//@ stdout: 104 3
#include <cstdio>
extern "C" int mf(char* p, int n) { return n; }
static inline __attribute__((overloadable)) int mf(char* const __attribute__((pass_object_size(1))) p, int n) {
  return int(__builtin_object_size(p, 1)) * 100 + n;
}
namespace std { using ::mf; }
using std::mf;
int main() {
  char b[1];
  int (*fp)(char*, int) = mf;  // the pass_object_size overload has no address
  std::printf("%d %d\n", mf(b, 4), fp(b, 3));
}
