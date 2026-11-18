// GCC's __integer_pack(N): in a template, a built-in pack whose elements
// are the int values 0, 1, ..., N-1, expanded by an enclosing pack
// expansion (a template argument list, a braced-init-list, a fold). N is an
// integral constant expression of any integral or unscoped enumeration
// type; a dependent N is expanded at instantiation, and an N that is not
// constant is a substitution failure. Found running GCC's C++ testsuite.
// WP-1667.
//@ std: c++20
//@ expect: compile-ok
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __integer_pack
//@ strict-diags
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <class T, T... I> struct seq {};
template <class T, T N> using make_seq = seq<T, __integer_pack(N)...>;
static_assert(same<make_seq<int, 4>, seq<int, 0, 1, 2, 3>>);
static_assert(same<make_seq<long, 2>, seq<long, 0, 1>>);
static_assert(same<make_seq<unsigned long, 0>, seq<unsigned long>>);
static_assert(same<make_seq<unsigned char, 3>, seq<unsigned char, 0, 1, 2>>);
static_assert(same<make_seq<bool, true>, seq<bool, false>>);

enum E { two = 2 };
template <E N> using from_enum = seq<int, __integer_pack(N)...>;
static_assert(same<from_enum<two>, seq<int, 0, 1>>);

// N not dependent, inside a template.
template <class T> struct fixed { using type = seq<int, __integer_pack(3)...>; };
static_assert(same<fixed<void>::type, seq<int, 0, 1, 2>>);

// A function template's explicit arguments, a braced-init-list, a fold.
template <int... Ns> constexpr int sum() { return (0 + ... + Ns); }
template <int N> constexpr int call_sum() { return sum<__integer_pack(N)...>(); }
static_assert(call_sum<5>() == 10);
template <int N> constexpr int last() {
  constexpr int v[] = {__integer_pack(N)..., -1};
  return v[N - 1];
}
static_assert(last<7>() == 6);
template <int N> constexpr int fold() { return (__integer_pack(N) + ... + 0); }
static_assert(fold<4>() == 6);

// One expansion per element of an enclosing pack.
template <class... T> struct list {};
template <int... Ns> using seqs = list<make_seq<int, Ns>...>;
static_assert(same<seqs<1, 2>, list<seq<int, 0>, seq<int, 0, 1>>>);

// A non-constant N is a substitution failure.
template <int...> struct A {};
template <class T> constexpr int pick(int, A<__integer_pack(T::value)...> * = nullptr) { return 1; }
template <class T> constexpr int pick(...) { return 2; }
struct constant { static constexpr int value = 2; };
struct variable { static int value; };
static_assert(pick<constant>(0) == 1);
static_assert(pick<variable>(0) == 2);

// Not a keyword: the name stays an ordinary identifier elsewhere.
namespace other { int __integer_pack = 3; }
int use = other::__integer_pack;
