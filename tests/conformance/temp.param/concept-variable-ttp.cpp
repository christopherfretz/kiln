// [temp.param]/1-2 (P2841R7): concept-tt-parameter and variable-tt-parameter
// forms -- named, unnamed, packs, defaults ([temp.param]/1 grammar), and
// [temp.names]/1: a concept-name or variable template name as a template
// argument. A concept template parameter is usable as a type-constraint
// ([temp.param]/4), in a placeholder `C auto`, as a concept-id `C<T>`
// ([temp.names]/9) and in a fold; `V<T>` names a specialization of the
// argument variable template and constant evaluation reads through it
// (spec/sema-templates.md §2.1.1). WP-746.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
template <class T> concept Int = __is_same(T, int);
template <class T> concept Any = true;
template <class T> concept Integral = __is_integral(T);
template <class T, class U> concept Same = __is_same(T, U);
template <class T> constexpr int sz = sizeof(T) == 1 ? 1 : 9;

// Named, unnamed, packs, defaults, and a template-head on a variable-tt-parameter.
template <template <class> concept C> struct A1 {};
template <template <class> concept> struct A2 {};
template <template <class> concept... Cs> struct A3 {};
template <template <class> concept C = Int> struct A4 {};
template <template <class> auto V> struct B1 {};
template <template <class> auto> struct B2 {};
template <template <class> auto... Vs> struct B3 {};
template <template <class> auto V = sz> struct B4 {};
template <template <class T> requires true auto V> struct B5 {};
A1<Int> a1; A2<Any> a2; A3<Int, Any> a3; A3<> a3e; A4<> a4;
B1<sz> b1; B2<sz> b2; B3<sz, sz> b3; B4<> b4; B5<sz> b5;

// Concept-ids with a concept template parameter.
template <template <class> concept C, class T> concept Sat = C<T>;
static_assert(Sat<Int, int>);
static_assert(!Sat<Int, long>);
template <template <class> concept C, class... Ts> concept all_of = (C<Ts> && ...);
static_assert(all_of<Integral, int, long, char>);
static_assert(!all_of<Integral, int, float>);
template <class T, template <class> concept... Cs> concept every = (Cs<T> && ...);
static_assert(every<int, Int, Any>);
static_assert(!every<long, Int, Any>);
static_assert(every<long>);
template <template <class> concept C, class T> concept Fw = every<T, C>;
static_assert(Fw<Int, int>);
static_assert(!Fw<Int, char>);

// Type-constraints and placeholders.
template <template <class> concept C> struct S {
  template <C T> static constexpr int f(T) { return 1; }
  static constexpr int h(C auto) { return 2; }
  static constexpr bool b = C<int>;
};
static_assert(S<Int>::f(1) == 1);
static_assert(S<Int>::h(1) == 2);
static_assert(S<Int>::b && S<Integral>::b);
template <template <class> concept C = Int> constexpr bool d = C<int>;
static_assert(d<> && d<Integral>);

// Variable template template parameters.
template <template <class> auto V, class T> constexpr int get = V<T>;
static_assert(get<sz, char> == 1 && get<sz, int> == 9);
template <template <class> auto V> struct W { static constexpr int v = V<char>; };
static_assert(W<sz>::v == 1);
template <template <class> auto... Vs> constexpr int sum = (0 + ... + Vs<char>);
static_assert(sum<sz, sz, sz> == 3);
