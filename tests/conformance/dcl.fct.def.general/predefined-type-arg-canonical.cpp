// [dcl.fct.def.general]/8 (implementation-defined `__PRETTY_FUNCTION__`,
// clang's PredefinedExpr::ComputeName): the ` [T = ...]` list prints a
// type argument canonical with every name scoped, as clang stores the
// converted arguments: `(anonymous namespace)::` is kept, typedefs and
// alias templates are looked through, and a local class prints without
// its function. LLVM's `getTypeName<T>()` cuts the type name out of this
// string. Verified against clang 19.
//@ expect: compile-ok
//@ std: c++17
constexpr bool eq(const char* a, const char* b) {
  while (*a && *a == *b) {
    ++a;
    ++b;
  }
  return *a == *b;
}
namespace {
namespace N1 {
struct S1 {};
template <class T> struct TT {};
}  // namespace N1
}  // namespace
namespace A {
namespace {
union U {};
}
typedef int myint;
template <class T> using Alias = N1::TT<T>;
}  // namespace A
template <class D> constexpr const char* name() { return __PRETTY_FUNCTION__; }
template <class... D> constexpr const char* names() { return __PRETTY_FUNCTION__; }
static_assert(eq(name<N1::S1>(), "const char *name() [D = (anonymous namespace)::N1::S1]"), "");
static_assert(eq(name<A::U>(), "const char *name() [D = A::(anonymous namespace)::U]"), "");
static_assert(eq(name<A::myint>(), "const char *name() [D = int]"), "");
static_assert(eq(name<A::Alias<A::U>>(),
                 "const char *name() [D = (anonymous namespace)::N1::TT<A::(anonymous namespace)::U>]"),
              "");
static_assert(eq(names<const N1::S1&, N1::S1*, int N1::S1::*>(),
                 "const char *names() [D = <const (anonymous namespace)::N1::S1 &, "
                 "(anonymous namespace)::N1::S1 *, int (anonymous namespace)::N1::S1::*>]"),
              "");
int main() {
  struct Local {};
  static_assert(eq(name<Local>(), "const char *name() [D = Local]"), "");
}
