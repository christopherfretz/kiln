// [over.match.ctor]/1, [class.conv.fct]/1, [over.ics.user]/4: a class `D`
// initialized from an expression of type `D` (or of a class derived from
// `D`) is initialized by `D`'s constructors; `D`'s conversion function
// template `operator T()` deduced with `T = D` is a non-viable candidate
// (clang `ovl_fail_trivial_conversion`), so it never beats the copy or
// move constructor and its definition, ill-formed for every `T`, is never
// instantiated ([temp.inst]/11). Direct-list-initialization of a member
// (the reduction of a JSON library's result wrapper), with and without a
// move constructor, copy- and direct-initialization, a derived-class
// source, an explicit and a non-explicit template. Matches clang 19.
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
