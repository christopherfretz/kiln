// [temp.dep.type]/1, [temp.local]/1, [expr.unary.op]/3: `&Scan::f` and
// `&Scan::x` inside a class template (or one of its partial
// specializations) are pointer-to-member constants whose class is the
// current instantiation `Scan<C>`; after instantiation they are pointers to
// members of the specialization. A member class of a class template forms
// `&In::g` as a pointer to member of that member class (WP-608).
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 5
#include <cstdio>

template <class C> struct Scan {
  int x = 2;
  int eat() { return 1; }
  int (Scan::*pf)();
  int Scan::*px;
  Scan() {
    pf = &Scan::eat;
    px = &Scan::x;
    static_assert(__is_same(decltype(&Scan::eat), int (Scan<C>::*)()));
    static_assert(__is_same(decltype(&Scan::x), int Scan<C>::*));
  }
  struct In {
    int g() { return 4; }
    static int (In::*get())() { return &In::g; }
  };
};

template <class C, class D> struct Two;
template <class C> struct Two<C, int> {
  int h() { return 3; }
  static int (Two::*get())() { return &Two::h; }
};

template <class C> struct Plain {
  int k() { return 5; }
  int call() {
    auto p = &Plain::k;
    return (this->*p)();
  }
};

int main() {
  Scan<char> s;
  Two<char, int> t;
  Scan<char>::In in;
  Plain<long> p;
  std::printf("%d %d %d %d %d\n", (s.*s.pf)(), s.*s.px, (t.*Two<char, int>::get())(),
              (in.*Scan<char>::In::get())(), p.call());
  return (s.*s.pf)() == 1 && s.*s.px == 2 ? 0 : 1;
}
