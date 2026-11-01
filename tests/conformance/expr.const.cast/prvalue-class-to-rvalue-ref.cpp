// [expr.const.cast]/3: a prvalue of class type can be converted to an
// xvalue of a similar type with `const_cast<T&&>`, after temporary
// materialization (libc++'s `indirectly_writable` concept casts a proxy
// reference prvalue to `const iter_reference_t<Out>&&`).
//@ std: c++26
struct P {
  int x;
  constexpr const P &operator=(int v) const { return *this; }
};
constexpr P mk() { return P{3}; }
static_assert(__is_same(decltype(const_cast<const P &&>(mk())), const P &&));
static_assert(__is_same(decltype(const_cast<P &&>(mk())), P &&));
static_assert(const_cast<const P &&>(mk()).x == 3);
static_assert(const_cast<P &&>(P{4}).x == 4);

template <class T>
concept writable = requires(T t) { const_cast<const T &&>(t) = 1; };
static_assert(writable<P>);

template <class R>
concept proxy_write = requires(R (*f)()) { const_cast<const R &&>(f()) = 1; };
static_assert(proxy_write<P>);
