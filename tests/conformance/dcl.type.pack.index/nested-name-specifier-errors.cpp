// [dcl.type.pack.index]/2 (C++26, P2662): the index of a pack-index-specifier
// used as a nested-name-specifier must be less than the pack's size, checked
// when the pack is substituted; the qualifier then names its member as usual
// ([expr.prim.id.qual]); the name before `...[` must name a pack.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:51 error "invalid index 2 for pack 'Ts' of size 2"
//@ diag: 15:1 note "in instantiation of template type alias 'Bad' requested here"
//@ diag: 16:64 error "no type named 'missing' in 'A'"
//@ diag: 17:6 note "in instantiation of template class 'M<A>' requested here"
//@ diag: 18:50 error "'T' does not refer to the name of a parameter pack"
struct A { using type = int; };
struct B { using type = long; };
template <class... Ts> using Bad = typename Ts...[2]::type;
Bad<A, B> bad;
template <class... Ts> struct M { using t = typename Ts...[0]::missing; };
M<A> m;
template <class T> struct P { using t = typename T...[0]::type; };
