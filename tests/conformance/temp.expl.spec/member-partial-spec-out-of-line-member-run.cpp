// [temp.expl.spec]/16, [temp.spec.partial.member]/1, [temp.mem]/1: a member
// of a partial specialization of a member template of an implicitly
// instantiated class template specialization is defined out of line with
// the partial specialization's own head. kiln accepts and matches the
// partial specialization (parameter names may differ; the deduced T and c
// reach the body); clang 19 and 22 reject it ("nested name specifier ...
// does not refer into a class"), which kiln treats as a clang defect:
// documented deviation D7 in spec/sema-templates.md section 1.5.
//@ expect: run
//@ skip(clang): clang 19/22 reject the out-of-line member (deviation D7)
//@ stdout: 100 0 0 97
#include <cstdio>
template <class... Ts> struct O {
  template <class T, char c> struct A { int f() { return 0; } };
};
template <> template <class T, char c> struct O<int, char>::A<T*, c> {
  int f();
  static constexpr int g();
};
template <> template <class U, char d> int O<int, char>::A<U*, d>::f() { return d + sizeof(U); }
template <> template <class T, char c> constexpr int O<int, char>::A<T*, c>::g() { return c; }
static_assert(O<int, char>::A<long*, 'a'>::g() == 'a');
int main() {
  std::printf("%d %d %d %d\n", O<int, char>::A<short*, 'b'>().f(), O<int, char>::A<short, 'b'>().f(),
              O<long, char>::A<short*, 'b'>().f(), O<int, char>::A<int*, 'a'>::g());
}
