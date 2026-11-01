// [temp.func.order]/3-5, [temp.deduct.partial]/8,11: partial ordering with
// function parameter packs. A trailing pack of the parameter template
// deduces from each remaining type; a pack of the argument template does
// not match a parameter that is not a pack; when both are at least as
// specialized, the template without a trailing pack wins over one whose
// pack has no corresponding parameter (CWG1395). libc++'s
// `__invoke(_Args&&...)` fallback versus `__invoke(_Fp&&, _Args&&...)`.
//@ expect: compile-ok
//@ strict-diags
template <int N> struct R { static constexpr int v = N; };

template <class... A> R<1> f(A&&... a);
template <class F, class... A> R<2> f(F&& x, A&&... a);
template <class F, class G, class... A> R<3> f(F&& x, G&& y, A&&... a);

static_assert(decltype(f())::v == 1);
static_assert(decltype(f(1))::v == 2);
static_assert(decltype(f(1, 'c'))::v == 3);
static_assert(decltype(f(1, 'c', 2.0, nullptr))::v == 3);

template <class... A> R<1> h(A... a);
template <class F> R<2> h(F x);
template <class F, class... A> R<3> h(F x, A... a);
static_assert(decltype(h(1))::v == 2);      // CWG1395: h(F) beats h(F, A...)
static_assert(decltype(h(1, 2))::v == 3);

template <class T, class... A> R<1> p(T* t, A... a);
template <class T, class... A> R<2> p(T t, A... a);
static_assert(decltype(p((int*)0, 1, 2))::v == 1);
static_assert(decltype(p(0, 1))::v == 2);

template <class... A> R<1> q(int, A... a);
template <class... A> R<2> q(A... a);
static_assert(decltype(q(1, 2))::v == 1);

// Inside a template, with the libc++ shape (a SFINAE'd return type).
template <class T> T&& declval();
struct nat {};
template <class... A> nat inv(A&&... a);
template <class F, class... A>
constexpr decltype(declval<F>()(declval<A>()...)) inv(F&& f, A&&... a) {
  return static_cast<F&&>(f)(static_cast<A&&>(a)...);
}
struct Id {
  template <class T> constexpr T&& operator()(T&& t) const { return static_cast<T&&>(t); }
};
template <class It, class P>
constexpr It find(It first, It last, char v, P& proj) {
  for (; first != last; ++first)
    if (inv(proj, *first) == v)
      break;
  return first;
}
constexpr const char s[] = "abc";
Id id;
static_assert(find(s, s + 3, 'b', id) == s + 1);
