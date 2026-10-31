// [class.inhctor.init]/1, [dcl.constexpr]/3: an inheriting constructor is
// constexpr when the inherited constructor and the implicit initialization
// of the rest of the class satisfy the constexpr requirements; it is
// constant-evaluated with the forwarded arguments. Dependent bases are
// re-resolved at instantiation ([namespace.udecl]/3, libc++'s
// `using __base::__base;`).
//@ expect: compile-ok
//@ oven: steps<=64 calls<=32 bytes<=128
struct A { constexpr A(int x, int y = 7) : s(x + y) {} int s; };
template <class T> struct B : T { using T::T; int z = 3; };
template <class T> struct C : A { using A::A; T t{}; };
template <class T> struct Base {
  constexpr Base(T v) : v(v) {}
  T v;
  constexpr T get() const { return v; }
};
template <class T> struct D : Base<T> {
  using __base = Base<T>;
  using __base::__base;
  using __base::get;
};
template <class T> struct E : Base<T> { using Base<T>::Base; };
struct F : A { using A::A; };
struct G : F { using F::F; int w = 9; };
constexpr B<A> b(1);
static_assert(b.s == 8 && b.z == 3);
constexpr C<int> c(2, 2);
static_assert(c.s == 4 && c.t == 0);
constexpr D<int> d(5);
static_assert(d.get() == 5);
constexpr E<long> e(6);
static_assert(e.v == 6);
constexpr G g(1, 1);
static_assert(g.s == 2 && g.w == 9);
constexpr int sum() {
  G x(3);
  D<int> y = 4;
  return x.s + x.w + y.get();
}
static_assert(sum() == 23);
