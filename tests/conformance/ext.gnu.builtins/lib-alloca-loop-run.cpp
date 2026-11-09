//@ expect: run
//@ args: -std=gnu++26
//@ stdout: a16 b17 c18 d19 e20 1
// GNU alloca (clang EmitBuiltinExpr, WP-1073): in the GNU dialects the
// library function alloca is a builtin lowered to a dynamic `alloca` where
// the call is, aligned to __BIGGEST_ALIGNMENT__; each call in a loop gets
// distinct storage that lives until the function returns. The name is
// parenthesized so <alloca.h>'s macro does not replace the call.
#include <alloca.h>
#include <cstdio>
#include <cstring>

__attribute__((noinline)) void fill(char **ptrs, int n) {
  for (int i = 0; i < n; ++i) {
    ptrs[i] = static_cast<char *>((alloca)(17 + i));
    std::memset(ptrs[i], 'a' + i, 16 + i);
    ptrs[i][16 + i] = 0;
  }
  int aligned = 1;
  for (int i = 0; i < n; ++i) {
    std::printf("%c%zu ", ptrs[i][0], std::strlen(ptrs[i]));
    aligned &= reinterpret_cast<unsigned long>(ptrs[i]) % __BIGGEST_ALIGNMENT__ == 0;
  }
  std::printf("%d\n", aligned);
}

int main() {
  char *ptrs[5];
  fill(ptrs, 5);
}
