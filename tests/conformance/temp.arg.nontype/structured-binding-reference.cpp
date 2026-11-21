// [temp.arg.nontype]/2-3, [dcl.struct.bind]/5-7: the name of a structured
// binding is an lvalue naming an element or member of the decomposed
// variable e; when e has static storage duration (at namespace scope,
// constexpr or not), that subobject is a valid argument for a template
// parameter of reference or pointer type, and arguments naming the same
// subobject are the same (clang 22 accepts; GCC 16.1 rejects the
// constexpr decomposition's, "the address of '<structured bindings>' is
// not a valid template argument"). reflect_object of a binding rests on
// this (meta.reflection.result/reflect-object-structured-binding.cpp,
// WP-1783).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
struct P { int x; int y; };
P gp{1, 2};
auto [a, b] = gp;
constexpr P cp{3, 4};
constexpr auto [ca, cb] = cp;
int arr[2] = {5, 6};
auto [e0, e1] = arr;

template <int& R> struct X { static constexpr int* p = &R; };
template <const int& R> struct C { static constexpr int v = R; };
template <const int* Q> struct Y { static constexpr const int* p = Q; };

static_assert(X<a>::p == &a && X<e1>::p == &e1 && X<b>::p != &a);
static_assert(C<ca>::v == 3 && C<cb>::v == 4);
static_assert(Y<&cb>::p == &cb);
static_assert(__is_same(X<a>, X<a>) && !__is_same(X<a>, X<b>));
