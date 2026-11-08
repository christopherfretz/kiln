// [expr.unary.op]/3, [temp.arg.explicit]/4: in a member function of a
// class template, `&C::f<A>` (also through an alias `Self`) names a
// specialization of a member function template and forms a pointer to
// member, at definition and again at instantiation. It is not
// `&this->f<A>`. A static member template's specialization is an
// ordinary function pointer. Reduced from clang's
// unittests/StaticAnalyzer/RangeSetTest.cpp:
// `wrap(&Self::checkIntersectImpl<const llvm::APSInt &>, ...)`.
//@ expect: run
//@ stdout: 3 4 5 6 1
#include <cstdio>

struct A {
  int v;
};

template <class B> struct T {
  using Self = T<B>;
  B base;
  template <class P> int impl(P p) { return int(p.v + base); }
  template <class P> static int simpl(P p) { return p.v + 3; }
  template <class F, class... Args> int wrap(F f, Args... a) {
    return (this->*f)(a...);
  }
  void run() {
    A a{3};
    int x = wrap(&Self::impl<const A &>, a);
    int y = wrap(&T::impl<A>, A{4});
    int (T::*pm)(A) = &T<B>::template impl<A>;
    int (*pf)(A) = &Self::simpl<A>;
    std::printf("%d %d %d %d %d\n", x, y, (this->*pm)(A{5}), pf(a), pm != nullptr);
  }
};

int main() {
  T<int> t{0};
  t.run();
}
