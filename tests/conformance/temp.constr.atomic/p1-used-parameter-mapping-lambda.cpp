// [temp.constr.atomic]/1, [temp.constr.order]: an atomic constraint's
// parameter mapping covers only the template parameters that appear within
// its expression, including those inside a lambda-expression (its captures,
// parameters, requires-clause and body), a statement expression in its
// body, a local class or enumeration declared there, and a closure type
// (clang `MarkUsedTemplateParameters` walks them). `C<T, int>` and
// `C<T, long>` then give the same atom and the overload adding a conjunct
// is more constrained; a lambda that names `U` keeps the atoms apart
// (ambiguous). WP-821.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 65:9 error "call to 'g' is ambiguous"
//@ diag: 63:54 note "candidate function [with T = int]"
//@ diag: 64:63 note "candidate function [with T = int]"
template <class T, class U> concept L1 = [] { return sizeof(T) > 1; }();
template <class T> requires L1<T, int> constexpr int f1(T) { return 1; }
template <class T> requires L1<T, long> && true constexpr int f1(T) { return 2; }
static_assert(f1(1L) == 2);

template <class T, class U> concept L2 = requires { [](T, T* = nullptr) {}; };
template <class T> requires L2<T, int> constexpr int f2(T) { return 1; }
template <class T> requires L2<T, long> && true constexpr int f2(T) { return 2; }
static_assert(f2(1L) == 2);

template <class T, class U> concept L3 = [] { return ({ T t{}; sizeof(t); }) > 1; }();
template <class T> requires L3<T, int> constexpr int f3(T) { return 1; }
template <class T> requires L3<T, long> && true constexpr int f3(T) { return 2; }
static_assert(f3(1L) == 2);

template <class T, class U>
concept L4 = [n = sizeof(T)] { enum E { e = sizeof(T) }; return n == e; }();
template <class T> requires L4<T, int> constexpr int f4(T) { return 1; }
template <class T> requires L4<T, long> && true constexpr int f4(T) { return 2; }
static_assert(f4(1L) == 2);

template <class T, class U>
concept L5 = [](T x) noexcept(sizeof(T) > 0) requires(sizeof(T) > 0) { return sizeof(x) > 1; }(T{});
template <class T> requires L5<T, int> constexpr int f5(T) { return 1; }
template <class T> requires L5<T, long> && true constexpr int f5(T) { return 2; }
static_assert(f5(1L) == 2);

template <class T, class U> concept L6 = [] {
  struct S : T { T t; constexpr int get() const { return sizeof(t); } };
  return S{}.get() > 0;
}();
struct B {};
template <class T> requires L6<T, int> constexpr int f6(T) { return 1; }
template <class T> requires L6<T, long> && true constexpr int f6(T) { return 2; }
static_assert(f6(B{}) == 2);

template <class T, class U> concept L7 = [] {
  auto g = [] { return T{}; };
  using R = decltype(g());
  static_assert(sizeof(R) > 0);
  return sizeof(g()) > 1;
}();
template <class T> requires L7<T, int> constexpr int f7(T) { return 1; }
template <class T> requires L7<T, long> && true constexpr int f7(T) { return 2; }
static_assert(f7(1L) == 2);

// the lambda names U: the atoms differ
template <class T, class U> concept M = [] { return sizeof(T) >= sizeof(U); }();
template <class T> requires M<T, char> constexpr int g(T) { return 1; }
template <class T> requires M<T, short> && true constexpr int g(T) { return 2; }
int q = g(1); // ambiguous
