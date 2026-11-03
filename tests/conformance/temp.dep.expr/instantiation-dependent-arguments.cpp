// [temp.dep.expr]/4, [temp.dep.constexpr]/2, [temp.res.general]/6: `TC<N>`
// with `N` a constant local of a function template is instantiation-dependent
// but not dependent. A concept-id, type trait or default argument over it is
// not value-dependent: it is checked at the definition (clang
// `anyDependentTemplateArguments` tests `isDependent`), and a specialization
// it names is shared with every other use of the same canonical arguments,
// so another function template's instantiation never meets the first one's
// local (libc++ `subrange(from, to)` in the range insertion tests).
//@ expect: compile-ok
//@ strict-diags
template <int N> struct TC { int x; };
template <class I> concept C = sizeof(I) == sizeof(void*);
template <class I, bool K = C<I>> struct A { static constexpr bool k = K; };
template <class T> constexpr bool is_obj = __is_object(T);
template <class T> concept Obj = is_obj<T>;

template <class U> void g() {
  constexpr int N = 3;
  using T = TC<N>;
  A<T*> s;
  static_assert(s.k);
  static_assert(__is_object(T*));
  static_assert(is_obj<T*>);
  static_assert(Obj<T*>);
}
template void g<int>();

template <class U> void h() {
  constexpr int N = 4;
  using T = TC<N>;
  bool b = is_obj<T*>;
  (void)b;
}
static_assert(is_obj<TC<4>*>);

template <class S, class I> concept D = requires(S a, I b) { a - b; };
enum class K : bool { u, s };
template <class I, class S = I, K k = D<S, I> ? K::s : K::u>
  requires(k == K::s || !D<S, I>)
struct R { R(I, S) {} };
template <class I, class S> R(I, S) -> R<I, S>;
template <class U> void r1() {
  constexpr int N = 5;
  using T = TC<N>;
  T* p = nullptr;
  auto x = R(p, p);
  (void)x;
}
template <class U> void r2() {
  constexpr int N = 5;
  using T = TC<N>;
  T* p = nullptr;
  auto x = R(p, p);
  (void)x;
}
template void r2<int>();
