//@ expect: run
//@ stdout: 1 0 1 2 1
// [basic.scope.param]/1, [temp.constr.decl]/3.2: a function parameter is in
// scope in the function's trailing requires-clause, which is checked with
// the parameters' types substituted (libc++'s `reverse_iterator`
// `operator==`, which requires `__x.base() == __y.base()`). WP-121.
extern "C" int printf(const char *, ...);
template <class T> struct R {
  T v;
  constexpr T base() const { return v; }
};
struct Opaque {};
template <class A, class B>
constexpr bool operator==(const R<A> &x, const R<B> &y)
  requires requires { x.base() == y.base(); }
{
  return x.base() == y.base();
}
template <class T> constexpr int pick(T a) requires(sizeof(a) > 1) { return 2; }
template <class T> constexpr int pick(T) { return 1; }
template <class T> struct Box {
  T t;
  template <class U> constexpr bool eq(U u) const requires requires { t == u; } { return t == u; }
  constexpr int size(int n) const requires(sizeof(T) > 1) { return n; }
};
template <class X, class Y>
concept Comparable = requires(X x, Y y) { x == y; };
static_assert(Comparable<R<int>, R<long>>);
static_assert(!Comparable<R<Opaque>, R<int>>);
static_assert(R<int>{3} == R<long>{3});
static_assert(pick('c') == 1 && pick(5) == 2);
static_assert(Box<int>{4}.eq(4L));
int main() {
  R<int> a{1};
  R<long> b{1}, c{2};
  printf("%d %d %d %d %d\n", a == b, a == c, pick('x'), pick(7), Box<short>{1}.size(1));
}
