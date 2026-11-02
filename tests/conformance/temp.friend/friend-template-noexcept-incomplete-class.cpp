// [class.mem.general]/7, [temp.inst]/14, [except.spec]/13: the dependent
// noexcept-specifier of a friend template declared in a class template is
// a complete-class context and is substituted only when needed, whether the
// friend is defined in the class or only declared there. Instantiating the
// class (here while an enclosing specialization is itself incomplete) does
// not look into members declared after the friend (libc++
// counted_iterator/move_iterator `iter_swap`).
template <class T> concept C = true;
template <class T, class U> void nothrow_swap(T&, U&) noexcept;
template <class T, class U> void throwing_swap(T&, U&);

template <class I> struct counted {
  template <C I2>
  friend constexpr void defined(const counted& x, const counted<I2>& y) noexcept(noexcept(nothrow_swap(x.cur_, y.cur_))) {}
  template <class I2>
  friend void declared(const counted& x, const counted<I2>& y) noexcept(noexcept(nothrow_swap(x.cur_, y.cur_)));
  template <class I2>
  friend void declared_throws(const counted& x, const counted<I2>& y) noexcept(noexcept(throwing_swap(x.cur_, y.cur_)));
  I cur_;
};

// counted<counted<int>> is instantiated while wrap<int> is incomplete, and
// counted<int> while counted<counted<int>> is.
template <class I> struct wrap { counted<counted<I>> c; };
wrap<int> w;
counted<int> c;
counted<long> d;
static_assert(noexcept(defined(c, d)));
static_assert(noexcept(declared(c, d)));
static_assert(!noexcept(declared_throws(c, d)));
static_assert(noexcept(declared(w.c, w.c)));

// A friend template redeclaring a namespace-scope template keeps that
// template's specification.
template <class T> struct B { static constexpr bool v = sizeof(T) == 4; };
template <class I> struct S { template <class T> friend void g(T) noexcept(B<T>::v); };
template <class T> void g(T) noexcept(B<T>::v) {}
S<int> s;
static_assert(noexcept(g(1)) && !noexcept(g('a')));
int main() {}
