// [dcl.attr.grammar]/4, /6 (an attribute-namespace names an
// implementation-defined attribute): clang's sanitizer attributes and
// `lifetimebound` are known and ignored in their clang spellings (WP-667;
// spec/sema.md §4.5). `no_sanitize`, `no_sanitize_memory` and
// `lifetimebound` are `Clang<"x">` (`__attribute__((x))`, `[[clang::x]]`);
// `no_sanitize_address`, `no_sanitize_thread` and
// `no_address_safety_analysis` are `GCC<"x">` (`__attribute__((x))`,
// `[[gnu::x]]`). No diagnostic, even under `-Werror -Wunknown-attributes`.
// Reduced from an open-source formatting library's `reserve` and `join`.
//@ expect: run
//@ strict-diags
//@ args: -Werror -Wunknown-attributes
//@ stdout: 7 3

#include <cstdio>

template <typename T>
__attribute__((no_sanitize("undefined"))) inline T *reserve(T *p, unsigned n) {
  return p + n;
}

[[clang::no_sanitize("address", "undefined")]] int f1() { return 1; }
__attribute__((no_sanitize_memory)) int f2() { return 2; }
[[clang::no_sanitize_memory]] int f3() { return 3; }
__attribute__((no_sanitize_address)) int f4() { return 4; }
[[gnu::no_sanitize_address]] int f5() { return 5; }
[[gnu::no_sanitize_thread]] int f6() { return 6; }
__attribute__((no_address_safety_analysis)) int f7() { return 7; }
__attribute__((__no_sanitize__("undefined"))) int f8() { return 8; }

struct view {
  const int *p;
};
template <typename T> view join(const T &t [[clang::lifetimebound]]) { return {&t}; }
const int &pick(const int &a __attribute__((lifetimebound)), const int &) { return a; }

int main() {
  int buf[8] = {0, 1, 2, 3, 4, 5, 6, 7};
  int x = 3;
  int s = f1() + f2() + f3() + f4() + f5() + f6() + f7() + f8();
  std::printf("%d %d\n", *reserve(buf, 7) + (s - 36), *join(pick(x, s)).p);
}
