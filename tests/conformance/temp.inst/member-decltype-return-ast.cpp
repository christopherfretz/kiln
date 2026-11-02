//@ expect: ast
//@ oracle: clang
// [temp.inst]/3, [dcl.type.decltype]/1, [expr.type.conv]/2: a member's
// `decltype` trailing return type keeps its expression in an implicit class
// template instantiation; a substituted `T()` is a value-initialization of the
// argument type and prints as `int()`, and a parameter named in its own
// declarator's trailing return type prints by name (`decltype(t.x)`), as clang's
// dump.
using I = int;
template <class T> struct A {
  template <class U> auto f(U u) -> decltype(u + T());
  auto g(T t) -> decltype(t + T());
  auto h(T t) -> decltype(t + I() + T{} + (T)0);
};
A<int> a;
A<long> b;
struct P { int x; };
template <class T> struct B { auto k(T t) -> decltype(t.x); };
B<P> c;
