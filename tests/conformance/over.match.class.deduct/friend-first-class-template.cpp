// [over.match.class.deduct]/1, [temp.friend]/1, [temp.param]/12: class
// template argument deduction forms its guides from the template's own
// (namespace scope, depth 0) parameters and defaults even when its first
// declaration is a friend declaration inside a class template, whose
// parameters sit one level deeper (libc++ 19 declares `basic_string_view`
// first as a friend of `__wrap_iter`). Implicit guides from constructors
// and constructor templates, a user-declared guide, the aggregate
// deduction candidate and the copy deduction candidate, with the friend
// one and two class template levels deep. Verified against clang 19
// (WP-299).
//@ expect: run
//@ exit: 115
template <class C> struct traits { static constexpr int v = sizeof(C) * 10; };
template <class I> class wrap {
  template <class C, class Tr> friend class sv;
  template <class C, class Tr> friend struct agg;
};
template <class T> struct outer {
  template <class X> struct inner {
    template <class C, class Tr> friend class deep;
  };
};
template <class C, class Tr = traits<C>> class sv {
public:
  C c;
  sv(C x) : c(x) {}
  sv(C* p) : c(*p) {}
  template <class X> sv(X*, C y) : c(y) {}
  int get() const { return Tr::v; }
};
template <class X> sv(X*) -> sv<X>;
template <class C, class Tr = traits<C>> struct agg {
  C c;
  int get() const { return Tr::v; }
};
template <class C, class Tr = traits<C[2]>> class deep {
public:
  deep(C) {}
  int get() const { return Tr::v; }
};
wrap<int> w;

int main() {
  int i = 0;
  sv a('c');         // 10
  sv b(&i, short(2)); // 20
  sv c = a;          // 10
  agg d{'c'};        // 10
  deep e('c');       // 20
  sv<long> f(1L);
  sv g(&i);          // 40, the user-declared guide
  return a.get() + b.get() + c.get() + d.get() + e.get() + (f.get() == 80 ? 5 : 0) + g.get();
}
