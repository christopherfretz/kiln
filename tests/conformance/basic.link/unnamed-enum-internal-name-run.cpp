// [basic.link]/4, [dcl.enum]: an unnamed enumeration without a typedef
// name for linkage is not externally visible, so clang mangles it as the
// source name `$_N` (`mangleUnqualifiedName`; `Ut_` only for an externally
// visible unnamed type). Kiln writes `Ut_`, so its `typeid` name and the
// names of templates instantiated with it (profile records in fmt's
// coverage run) differ from clang's. Found by the fmt coverage comparison
// (WP-941).
//@ expect: run
//@ xfail(kiln): WP-941 follow-up, a non-externally-visible unnamed enumeration mangles as Ut_ instead of clang's $_N
//@ skip(gcc): the `$_N` spelling is clang's
//@ stdout: N1d3$_0E
#include <cstdio>
#include <typeinfo>
namespace d { enum { e = 1 }; }
int main() { std::puts(typeid(d::e).name()); }
