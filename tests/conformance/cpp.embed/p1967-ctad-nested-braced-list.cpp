// [cpp.embed] in an aggregate deduction ([over.match.class.deduct]/1.6,
// [temp.deduct.call]/1): the bytes of `#embed` fill a two-dimensional
// array member with brace elision (one parameter per element) or inside
// nested braced lists (each element list deduced against `T[N]`).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++26
//@ args: -Wno-c23-extensions
//@ strict-diags
//@ diag: 42:3 error "no viable constructor or deduction guide for deduction of template arguments of 'E'"
//@ diag: 21:27 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 21:27 note "implicit deduction guide declared as 'template <class T> E(E<T>) -> E<T>'"
//@ diag: 21:27 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
//@ diag: 21:27 note "implicit deduction guide declared as 'template <class T> E(T (&&)[2][2]) -> E<T>'"
//@ diag: 21:27 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 21:27 note "implicit deduction guide declared as 'template <class T> E() -> E<T>'"

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <class T> struct E { T t[2][2]; };
template <class T> struct F { T t[1][4]; };

constexpr E e1{
#embed "Inputs/p1967-data.txt"
};
constexpr E e2{{{
#embed "Inputs/p1967-data.txt" limit(2)
}, {
#embed "Inputs/p1967-data.txt" limit(2)
}}};
F f1{
#embed "Inputs/p1967-data.txt"
};
constexpr F f2{{{
#embed "Inputs/p1967-data.txt"
}}};
static_assert(same<decltype(e1), const E<int>> && same<decltype(e2), const E<int>>);
static_assert(same<decltype(f1), F<int>> && same<decltype(f2), const F<int>>);
static_assert(e2.t[1][0] == e1.t[0][0] && f2.t[0][3] == e1.t[1][1]);

E bad{{{
#embed "Inputs/p1967-data.txt" limit(2)
}, {1L, 2L}}};
