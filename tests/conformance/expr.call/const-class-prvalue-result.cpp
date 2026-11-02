// [expr.call]/14, [expr.type]/2: a call returning `const X` (X a class)
// is a prvalue of type `const X`; only non-class prvalues lose their
// cv-qualifiers. libc++'s lazy_split test relies on
// `forward_iterator` rejecting a `const It operator++(int)` (WP-276).
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
template <class A, class B> concept same_as = same<A, B>;

struct X {
  const X g() const { return *this; }
  const X operator++(int) { return *this; }
  const X operator+(int) const { return *this; }
};
const X f();
const int h();
X x;

static_assert(same<decltype(f()), const X>);
static_assert(same<decltype(h()), int>);
static_assert(same<decltype(x.g()), const X>);
static_assert(same<decltype(x + 1), const X>);
static_assert(same<decltype(x++), const X>);

template <class I> concept postinc_same = requires(I i) {
  { i++ } -> same_as<I>;
};
static_assert(!postinc_same<X>);
static_assert(postinc_same<int*>);

template <class T> const T make();
static_assert(same<decltype(make<X>()), const X>);
static_assert(same<decltype(make<int>()), int>);
