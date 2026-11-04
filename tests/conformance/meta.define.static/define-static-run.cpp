// [meta.define.static] (P3491R3), spec/reflection-values.md §4.4: the
// objects define_static_string, define_static_array and
// define_static_object promote are usable at run time. Each is emitted
// once (equal contents are one object, so s == s2 at run time too), with
// internal linkage, its initializer the frozen constant.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: run
//@ strict-diags
//@ stdout: hello, static
//@ stdout: 3 1 4
//@ stdout: 5 6
//@ stdout: 1
//@ stdout: 7 8
//@ stdout: tf 3
//@ stdout: tf 1
//@ stdout: tf 4
//@ stdout: wide 4
#include "kiln_meta.h"
extern "C" int puts(const char*);
extern "C" int printf(const char*, ...);
struct P { int x, y; };
constexpr int arr[] = {3, 1, 4};
constexpr const char* gs = std::define_static_string("hello, static");
const P* volatile gp = std::define_static_object(P{7, 8});
int main() {
  constexpr const char* s = std::define_static_string("hello, static");
  puts(s);
  constexpr auto sp = std::define_static_array(arr);
  printf("%d %d %d\n", sp[0], sp[1], sp[2]);
  constexpr const P* po = std::define_static_object(P{5, 6});
  printf("%d %d\n", po->x, po->y);
  const char* volatile s2 = gs;
  printf("%d\n", s == s2);
  printf("%d %d\n", gp->x, gp->y);
  template for (constexpr int v : std::define_static_array(arr)) {
    printf("tf %d\n", v);
  }
  constexpr const wchar_t* w = std::define_static_string(L"wide");
  int n = 0;
  while (w[n]) ++n;
  printf("wide %d\n", n);
  return 0;
}
