// [temp.alias]/2, [temp.dep.type]/9, [temp.deduct.general]/8: `void_t<E>`
// is not a dependent type (it is `void`) but it is instantiation-dependent,
// and so is `is_same<void, void_t<E>>::value` through its qualifier. As a
// template argument it is substituted again in each deduction, so a
// failure in `E` removes the candidate. It is not folded to `true` when
// the template is defined. This is libc++'s `operator<<(ostream&,
// unique_ptr const&)` constraint.
template <class...> using void_t = void;
template <class T, class U> struct is_same { static constexpr bool value = false; };
template <class T> struct is_same<T, T> { static constexpr bool value = true; };
template <bool, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template <class T> T&& declval();
struct S {};
struct P {};
struct Q {};
int operator<<(S&, Q);
template <class T> struct W { using pointer = T; };
template <class Y, enable_if_t<is_same<void, void_t<decltype(declval<S&>() << declval<typename W<Y>::pointer>())>>::value, int> = 0>
char f(S&, W<Y> const&);
template <class Y> long f(S&, Y const&);
static_assert(sizeof(f(declval<S&>(), declval<W<P>&>())) == sizeof(long));
static_assert(sizeof(f(declval<S&>(), declval<W<Q>&>())) == sizeof(char));
template <class Y, class = enable_if_t<is_same<void, void_t<typename Y::type>>::value>>
char g(Y);
long g(...);
static_assert(sizeof(g(P{})) == sizeof(long));
static_assert(sizeof(g(enable_if<true, int>{})) == sizeof(char));
// A local constant in a template is instantiation-dependent too; the
// argument still names the same specialization.
template <int N> struct X { static constexpr int v = N; };
template <class T> constexpr int h() { constexpr int n = 3; X<n> x; return x.v + X<n>::v; }
static_assert(h<int>() == 6);
