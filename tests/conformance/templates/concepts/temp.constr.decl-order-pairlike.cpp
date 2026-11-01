//@ expect: compile-ok
// [temp.constr.decl]/3: the associated constraints are the type-constraints
// of the template parameters, then the requires-clause, then the trailing
// requires-clause, and satisfaction stops at the first unsatisfied
// conjunct ([temp.constr.op]/3). A requires-clause that would be a hard
// error is never substituted when a type-constraint fails (the shape of
// libc++'s `template <__pair_like_no_subrange P> requires(... get<1>(...)
// ...) pair(P&&)` with `P = tuple<int>`).
template <class T> struct Bad {
  static_assert(sizeof(T) == 0, "instantiated");
  using type = int;
};
template <class T> concept Huge = sizeof(T) == 1000;
template <class T> typename Bad<T>::type get(T);

template <class A> struct P {
  template <Huge T>
    requires(sizeof(decltype(get(T()))) > 0)
  P(T &&);
  P(int);
};
struct X {};
static_assert(!__is_constructible(P<int>, X));

template <Huge T>
  requires(sizeof(decltype(get(T()))) > 0)
void f(T) requires(sizeof(typename Bad<T>::type) > 0);
void f(...);
void g() { f(X()); }

template <Huge T>
  requires(sizeof(decltype(get(T()))) > 0)
struct C {};
template <class T> concept HasC = requires { typename C<T>; };
static_assert(!HasC<X>);
