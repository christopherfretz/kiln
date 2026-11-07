// [expr.prim.this]/2, [temp.inst]/14: the deferred `noexcept(expr)` of a
// class template specialization's constructor, destructor or conversion
// function is instantiated with `this` (an implicit member access), as a
// member function's is; only a static member function has none
// (daw_json_link's `~json_writer_t() noexcept(noexcept(m_writer.put(' ')))`,
// WP-792).
// expected-no-diagnostics
template<class T> T&& dv() noexcept;
struct W { void put(char) noexcept; int get() const noexcept; };
struct V { void put(char); int get() const; };
template<class T> struct J {
  T m;
  J() noexcept(noexcept(m.put(' ')));
  ~J() noexcept(noexcept(m.put(' '))) {}
  explicit operator int() const noexcept(noexcept(m.get())) { return m.get(); }
};
static_assert(noexcept(dv<J<W>&>().~J()));
static_assert(!noexcept(dv<J<V>&>().~J()));
static_assert(noexcept(J<W>()));
static_assert(!noexcept(J<V>()));
static_assert(noexcept(static_cast<int>(dv<const J<W>&>())));
static_assert(!noexcept(static_cast<int>(dv<const J<V>&>())));
