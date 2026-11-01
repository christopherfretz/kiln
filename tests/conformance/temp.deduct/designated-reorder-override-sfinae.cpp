// [temp.deduct]/8, [temp.constr.atomic], [dcl.init.aggr]/3.1: designators out
// of declaration order and a designator initializing a subobject twice make
// the program ill-formed (clang accepts them as extensions with an ExtWarn
// marked SFINAEFailure), so in a substitution they are failures whatever the
// warning's severity, and a constraint using them is not satisfied. The
// C99-extension designators (nested, array, mixed) are not failures.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 20:63 warning "nested designators are a C99 extension"
//@ diag: 26:49 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 26:57 note "first non-designated initializer is here"
//@ diag: 28:50 warning "nested designators are a C99 extension"
//@ diag: 30:47 warning "array designators are a C99 extension"
struct S2 { int a, b; };
struct N { S2 s; int c; };
template <class T> concept Reorder = requires { T{.b = 1, .a = 2}; };
static_assert(!Reorder<S2>);
template <class T> concept Ovr = requires { T{.a = 1, .a = 2}; };
static_assert(!Ovr<S2>);
template <class T> concept Part = requires { T{.s = S2{1, 2}, .s.a = 2}; };
static_assert(!Part<N>);
union U { int a, b; };
template <class T> concept UnionOvr = requires { T{.a = 1, .b = 2}; };
static_assert(!UnionOvr<U>);
// The C99-extension warnings are not substitution failures.
template <class T> concept Mixed = requires { T{.a = 1, 2}; };
static_assert(Mixed<S2>);
template <class T> concept Nested = requires { T{.s.a = 1}; };
static_assert(Nested<N>);
template <class T> concept Arr = requires { T{[0] = 1}; };
static_assert(Arr<int[2]>);
template <class T> auto p(int) -> decltype(T{.b = 1, .a = 2}, 1);
template <class T> char p(long);
static_assert(sizeof(p<S2>(0)) == 1);
template <class T> auto q(int) -> decltype(T{.a = 1, .a = 2}, 1);
template <class T> char q(long);
static_assert(sizeof(q<S2>(0)) == 1);
template <class T> auto r(int) -> decltype(T{.a = 1, .b = 2}, 1);
template <class T> char r(long);
static_assert(sizeof(r<S2>(0)) == 4);
// -Wno does not make them valid.
#pragma clang diagnostic ignored "-Wreorder-init-list"
static_assert(!Reorder<S2>);
