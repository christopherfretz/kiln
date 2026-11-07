// [temp.deduct.general]/8, [dcl.init.list]/3.7: a narrowing conversion in
// a list-initialization makes the expression invalid, so a substitution
// that forms one fails. clang's narrowing diagnostics are `SFINAEFailure`
// ExtWarns: the substitution fails even when the warning is ignored, by
// `-Wno-narrowing` or `#pragma GCC diagnostic ignored "-Wnarrowing"`
// (a library trait that suppresses the GCC warning around its probe).
//@ args: -std=c++20 -Wno-c++11-narrowing
//@ strict-diags
#pragma GCC diagnostic ignored "-Wnarrowing"

template <class T> T &&declval();
struct true_type { static constexpr bool value = true; };
struct false_type { static constexpr bool value = false; };

struct W { explicit W(long v) : val(v) {} long val; };

template <class T, class C> struct is_direct_constructible {
  template <class TT, class CC>
  static auto test(int) -> decltype(TT{declval<CC>()}, true_type());
  template <class, class> static false_type test(...);
  static constexpr bool value = decltype(test<T, C>(0))::value;
};
static_assert(!is_direct_constructible<W, double>::value);   // type narrowing
static_assert(is_direct_constructible<W, int>::value);
static_assert(!is_direct_constructible<int, long>::value);   // variable narrowing

template <class T> concept list_from_double = requires(double d) { T{d}; };
static_assert(!list_from_double<int>);
static_assert(list_from_double<double>);

template <int N> struct A {};
template <class T> auto cce(int) -> A<T{300}>;               // constant narrowing
template <class T> false_type cce(...);
static_assert(__is_same(decltype(cce<signed char>(0)), false_type));

// Outside a substitution the ignored warning stays ignored.
long l = 1;
int i{l};
