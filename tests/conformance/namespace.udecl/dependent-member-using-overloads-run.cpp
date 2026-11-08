// [namespace.udecl]/11, [over.load]: in a class template, a dependent
// using-declarator (`using ea<P>::f;`) is assumed to introduce functions
// that overload with those a later non-dependent one (`using ed::f;`)
// introduces; a conflict could only be found at instantiation (clang's
// `CheckOverload` with an `UnresolvedUsingValueDecl`). Boost.Cobalt's
// promises bring in `await_transform` this way; kiln reported "target of
// using declaration conflicts with declaration already in scope" (WP-812).
//@ expect: run
//@ std: c++20
//@ stdout: 0 1 2
extern "C" int printf(const char*, ...);
template <class... A> struct dao {};
template <class P> struct ea {
  template <class Aw> int f(Aw&&, int = 0) { return 0; }
};
struct ed {
  template <class... Args, class I> int f(dao<void(Args...), I>) { return 1; }
};
struct ee {
  int f(int*) { return 2; }
};
template <class Y> struct gp : ea<gp<Y>>, ed, ee {
  using ea<gp<Y>>::f;
  using ed::f;
  using ee::f;
};
int main() {
  gp<int> g;
  printf("%d %d %d\n", g.f(1L), g.f(dao<void(int), int>{}), g.f((int*)nullptr));
}
