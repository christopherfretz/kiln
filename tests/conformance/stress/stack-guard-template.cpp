// [temp.inst]/16 (implementation-defined instantiation depth), spec/
// sema-templates.md §1 P4, §10.3: with -ftemplate-depth raised to 100000,
// the recursive instantiation of R<N>::v exhausts the native stack long
// before the depth limit; the stack guard stops with a fatal diagnostic and
// the instantiation stack instead of overflowing. The small
// -fkiln-stack-size makes the trip independent of the build profile (and
// keeps the stack touched small). The recursion has no base case: the guard
// trips before any could be reached.
//@ expect: compile-fail
//@ skip(clang): clang overflows its own stack or hits its depth limit
//@ args: -ftemplate-depth=100000 -fkiln-stack-size=32M
//@ diag: 15 fatal "kiln: stack exhausted during template instantiation"
template <int N>
struct R {
  static constexpr int v = R<N - 1>::v + 1;
};

int x = R<100000>::v;
