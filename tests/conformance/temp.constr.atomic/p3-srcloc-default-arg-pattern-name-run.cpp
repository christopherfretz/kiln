// [temp.constr.atomic]/3, [support.srcloc.cons]/2: a function template's
// constraints are checked from its pattern, so the function name a
// source-location default argument called in them observes is the
// pattern's, without the specialization's template-argument suffix (clang
// 22; clang 19 checked them from the specialization). Each overload pair
// picks 1 when the constraint saw the expected name (WP-1325).
//@ expect: run
//@ requires: clang>=22
//@ stdout: 1 1 1
#include <source_location>
extern "C" int printf(const char *, ...);
template <class U>
constexpr const char *nm(U, std::source_location l = std::source_location::current()) {
  return l.function_name();
}
constexpr bool eq(const char *a, const char *b) { return __builtin_strcmp(a, b) == 0; }
template <class T>
  requires(eq(nm(T{}), "int f(T)"))
int f(T) { return 1; }
int f(...) { return 2; }
template <char C> struct X { static constexpr bool value = C == 'i'; };
template <class T>
  requires(X<nm(T{})[4]>::value)
int i(T) { return 1; }
int i(...) { return 2; }
template <class R>
struct Ctx {
  template <class T>
    requires(eq(nm(T{}), "int Ctx<int>::m(T) [R = int]"))
  int m(T) { return 1; }
  int m(...) { return 2; }
};
int main() { printf("%d %d %d\n", f(0), i(0), Ctx<int>{}.m(0)); }
