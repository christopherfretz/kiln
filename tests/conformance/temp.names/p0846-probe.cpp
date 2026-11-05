// [temp.names]/3, [basic.lookup.argdep] (C++20, P0846R0): a name followed
// by `<` is a template-name when ordinary lookup finds nothing, so ADL can
// find a function template called with explicit arguments.
// WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
namespace N {
struct S {};
template <int I> int get(S) { return I; }
}
int use(N::S s) { return get<3>(s); }
int f();
int use2(N::S s) { return get<1>(s) + f(); }
