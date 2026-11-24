// [temp.deduct.general]/7, [temp.over.link]/5: an array bound whose
// value is known but whose expression is instantiation-dependent
// (`sizeof(sizeof(f(T())))` is not value-dependent, but names `T`) is
// kept on the constant array type and substituted in deduction, so a
// bound that is ill-formed for the deduced arguments is a substitution
// failure and the other overload is chosen. Also a kept bound inside a
// member template of a class template, and a pack in a kept bound
// ([temp.variadic]/5). Reduced from real code.
//@ expect: compile-ok
struct X {};
constexpr int f(int) { return 0; }

template <class T> constexpr int g(T, char (*)[sizeof(sizeof(f(T())))]) { return 1; }
constexpr int g(...) { return 2; }
static_assert(g(X(), nullptr) == 2);
static_assert(g(1, nullptr) == 1);

// The same bound through an alias template.
template <class T> using P = char (*)[sizeof(sizeof(f(T())))];
template <class T> constexpr int a(T, P<T>) { return 1; }
constexpr int a(...) { return 2; }
static_assert(a(X(), nullptr) == 2);
static_assert(a(1, nullptr) == 1);

// A member template of a class template: the outer argument is
// substituted with the class, the inner one in deduction.
template <class T> struct A {
  template <class U>
  static constexpr int g(U, char (*)[sizeof(sizeof(f(T()) + f(U())))]) { return 1; }
  static constexpr int g(...) { return 2; }
};
static_assert(A<int>::g(1, nullptr) == 1);
static_assert(A<int>::g(X(), nullptr) == 2);

// The value is the original's: the type is `char (*)[sizeof(size_t)]`.
template <class T> constexpr bool same(char (*)[sizeof(sizeof(f(T())))]) { return true; }
static_assert(same<int>(static_cast<char (*)[sizeof(sizeof(0))]>(nullptr)));

// A kept bound naming an unexpanded pack is a pack expansion's pattern.
template <class...> struct L {};
template <class... Ts> struct S { using t = L<char[sizeof(sizeof(Ts))]...>; };
static_assert(__is_same(S<int, long>::t, L<char[sizeof(sizeof(0))], char[sizeof(sizeof(0))]>));
