//@ expect: ast
//@ requires: clang>=22
// [temp.names]/1, [expr.prim.id.qual]: a template-id naming a variable
// template specialization through a nested-name-specifier keeps the
// qualifier as written, so types that spell the expression print it as
// clang does: `decltype(N::v<int>)`, `I_<S::w<char>>`, `decltype(::N::v<int>)`,
// and, dependent, `decltype(N::v<T>)` and `int[N::v<T>]`, also once
// instantiated (`I_<S::w<int>>`).
namespace N { template <class T> constexpr int v = sizeof(T); }
struct S { template <class T> static constexpr int w = 1; };
template <int I> struct I_ {};
using A = I_<N::v<int>>;
using B = decltype(N::v<int>);
using C = I_<S::w<char>>;
using D = decltype(::N::v<int>);
template <class T> struct P {
  using A = I_<N::v<int>>;
  using B = decltype(N::v<T>);
  using C = I_<S::w<T>>;
  int arr[N::v<T>];
};
P<int> p;
