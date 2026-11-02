// [temp.dep.type]/6, [namespace.udecl]/1, [class.inhctor.init]: in
// `using G::F::F;` inside a class template G with a dependent base
// F<U, T, int>, `G::F` names a member of an unknown specialization (the
// injected-class-name of the dependent base, found at instantiation), so the
// using-declarator names F's constructors and G inherits them.
//@ expect: compile-ok
//@ std: c++23
template <class X, class Y> constexpr bool same = false;
template <class X> constexpr bool same<X, X> = true;
template <typename T, typename U, typename V> struct F {
  constexpr F(T t, U u, V v) : t(t), u(u), v(v) {}
  T t; U u; V v;
};
template <typename T, typename U> struct G : F<U, T, int> { using G::F::F; };
G<char, bool> g0(true, 'a', 1);
static_assert(same<decltype(g0.t), bool>);
static_assert(same<decltype(g0.u), char>);
template <typename T> struct H : F<T, T, T> {
  using H::F::F;
  constexpr T sum() const { return this->t + this->u + this->v; }
};
constexpr int s = H<int>(1, 2, 3).sum();
static_assert(s == 6);
int main() { return g0.v == 1 ? 0 : 1; }
