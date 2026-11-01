// [expr.cond]/4: the operand types after class unification (libc++
// common_type<int, bad_reference_wrapper<int>> via the `const T&`
// conditional), and the two ill-formed outcomes: both conversions
// possible, or neither.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:42 error "conditional expression is ambiguous; 'S1' can be converted to 'S2' and vice versa"
//@ diag: 39:40 error "incompatible operand types ('E1' and 'E2')"
template <class T> T&& declval() noexcept;
template <class T, class U> struct is_same { static constexpr bool value = false; };
template <class T> struct is_same<T, T> { static constexpr bool value = true; };
template <class T> struct B {
  B(T&);
  B(T&&) = delete;
  operator T&() const;
};
using A1 = decltype(false ? declval<const int&>() : declval<const B<int>&>());
static_assert(is_same<A1, const int&>::value);
template <class X, class Y, class = void> struct ct { static constexpr int v = 0; };
template <class X, class Y> struct ct<X, Y, decltype((void)(false ? declval<X>() : declval<Y>()))> { static constexpr int v = 1; };
static_assert(ct<const int&, const B<int>&>::v == 1);
struct Base { int b = 1; };
struct Der : Base { int d = 2; };
Base gb; Der gd; const Base cgb;
using C1 = decltype(false ? gb : gd);
static_assert(is_same<C1, Base&>::value);
using C2 = decltype(false ? cgb : gd);
static_assert(is_same<C2, const Base&>::value);
using C3 = decltype(false ? Base() : gd);
static_assert(is_same<C3, Base>::value);
using C4 = decltype(false ? static_cast<Der&&>(gd) : static_cast<Base&&>(gb));
static_assert(is_same<C4, Base&&>::value);
struct Conv { operator int() const; };
using C5 = decltype(false ? Conv() : 1);
static_assert(is_same<C5, int>::value);
struct S1 {}; struct S2 { S2(S1); operator S1(); };
int amb(bool b, S1 s1, S2 s2) { (void)(b ? s1 : s2); return 0; }
struct E1 {}; struct E2 {};
int inc(bool b, E1 x, E2 y) { (void)(b ? x : y); return 0; }
