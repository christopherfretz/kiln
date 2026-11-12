// [temp.constr.atomic]/3, [expr.prim.req.general], [support.srcloc.cons]/2:
// what a source-location default argument observes when a function's
// constraints call it. In a requires-expression's requirements the call is
// in the requires-expression's body, which is no function: the function
// name is empty, as when the requires-expression is parsed (clang 19 and
// 22 both; clang's TransformRequiresExpr enters the RequiresExprBodyDecl).
// The line is the call's. Each overload pair picks 1 when the constraint
// saw the expected value (WP-1325).
//@ expect: run
//@ stdout: 1 1 1 1 1
#include <source_location>
extern "C" int printf(const char *, ...);
template <class U>
constexpr const char *nm(U, std::source_location l = std::source_location::current()) {
  return l.function_name();
}
template <class U>
constexpr const char *fn(U, const char *n = __builtin_FUNCTION()) { return n; }
template <class U>
constexpr unsigned ln(U, unsigned n = __builtin_LINE()) { return n; }

template <class T>
  requires requires { requires nm(T{})[0] == 0; }
int a(T) { return 1; }
int a(...) { return 2; }
template <class T>
  requires requires { requires fn(T{})[0] == 0; }
int b(T) { return 1; }
int b(...) { return 2; }
template <class T>
  requires requires { requires ln(T{}) == __LINE__; }
int c(T) { return 1; }
int c(...) { return 2; }
template <class R>
struct Ctx {
  template <class T>
    requires requires { requires nm(T{})[0] == 0 && fn(R{})[0] == 0; }
  int m(T) { return 1; }
  int m(...) { return 2; }
};
// A hidden friend's constraints are checked from the friend, which names
// the pattern's declaration (clang 19 and 22 both).
template <class R>
struct H {
  friend int g(H h)
    requires(__builtin_strcmp(nm(h), "int g(H<R>)") == 0)
  {
    return 1;
  }
};
int g(...) { return 2; }
int main() { printf("%d %d %d %d %d\n", a(0), b(0), c(0), Ctx<int>{}.m(0), g(H<int>{})); }
