// Mangling differential: implicit ABI tags (WP-384, clang `AbiTagState`).
// A dependent return type's tags count (`_Z1fB1xI1AENT_4typeEv`); a member
// template's name in a nested template prefix gets none
// (`_ZN1SI1AE1hIS0_EENT_4typeEv`); conversion functions, lambdas and
// function-pointer returns; a namespace's own tag is never written and
// counts as already emitted, and only an inline namespace's first
// declaration may carry one (spec/abi.md §8.15).
struct __attribute__((abi_tag("x"))) Tagged { int v; };
struct A { using type = Tagged; };
template <class T> typename T::type f() { return {}; }
template <class T> struct S { typename T::type g() { return {}; } template <class U> typename U::type h() { return {}; } };
struct C { operator Tagged() { return {}; } };
namespace __attribute__((abi_tag("ns"))) Q { struct In {}; In q() { return {}; } }
Q::In q2() { return {}; }
auto lam = [] { return Tagged(); };
Tagged (*fp())() { return nullptr; }
int use1() { f<A>(); S<A> s; s.g(); s.h<A>(); Tagged t = C(); Q::q(); q2(); lam(); fp(); return t.v; }

namespace __attribute__((abi_tag("a"))) NA { struct S {}; }
inline namespace __attribute__((abi_tag("b"))) { struct T {}; }
inline namespace I __attribute__((abi_tag("c"))) { struct U {}; }
namespace [[gnu::abi_tag("e")]] I { struct X {}; }
NA::S f1() { return {}; }
T f2() { return {}; }
U f3() { return {}; }
X f5() { return {}; }
int use2() { f1(); f2(); f3(); f5(); return 0; }
