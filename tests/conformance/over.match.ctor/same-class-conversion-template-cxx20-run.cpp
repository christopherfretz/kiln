// same-class-conversion-template-run.cpp under C++20: [class.conv.fct]/1
// does not depend on the language mode; the conversion function template
// is never a candidate for its own class. Matches clang 19.
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ stdout: copy,7
//@ stdout: move,8
//@ stdout: move,copy,copy,copy,
//@ stdout: copy,copy,copy,move,
extern "C" int printf(const char*, ...);
template <class T> constexpr bool never = false;
template <class T> T&& fwd(T& t) { return static_cast<T&&>(t); }

struct D {
  int v = 0;
  D() noexcept = default;
  D(const D& o) noexcept : v(o.v) { printf("copy,"); }
  D& operator=(const D&) noexcept = default;
  template <class T> explicit operator T() noexcept(false);
};
template <class T> D::operator T() noexcept(false) {
  static_assert(never<T>, "operator T instantiated");
  return T{};
}

struct M {
  int v = 0;
  M() noexcept = default;
  M(const M& o) noexcept : v(o.v) { printf("copy,"); }
  M(M&& o) noexcept : v(o.v) { printf("move,"); }
  template <class T> operator T() const { static_assert(never<T>, "operator T instantiated"); return T{}; }
};
struct MD : M {};

template <class T> struct B {
  B(T&& v, int e) : first{fwd<T>(v)}, second{e} {}
  T first;
  int second;
};
B<D> mk(D&& d) { return B<D>(fwd<D>(d), 0); }
B<M> mkm(M&& m) { return B<M>(fwd<M>(m), 0); }

int main() {
  D d;
  d.v = 7;
  B<D> b = mk(static_cast<D&&>(d));
  printf("%d\n", b.first.v);
  M m;
  m.v = 8;
  B<M> bm = mkm(static_cast<M&&>(m));
  printf("%d\n", bm.first.v);
  M x = static_cast<M&&>(m);  // copy-init from an xvalue: the move constructor
  M y = m;                    // copy-init from an lvalue: the copy constructor
  M z(m);
  M w{m};
  printf("\n");
  MD md;
  M p = md;                   // a derived-class source
  M q(md);
  M r{md};
  M s(static_cast<MD&&>(md));
  printf("\n");
  return x.v + y.v + z.v + w.v + p.v + q.v + r.v + s.v - 4 * 8;
}
