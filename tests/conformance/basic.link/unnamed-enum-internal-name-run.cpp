// [basic.link]/4, [dcl.enum]: an unnamed enumeration without a typedef
// name for linkage is not externally visible, so clang mangles it as the
// source name `$_N` (`mangleUnqualifiedName`; `Ut_` only for an externally
// visible unnamed type), as kiln does since WP-1651 (it wrote `Ut_`, so
// its `typeid` name and the names of templates instantiated with it,
// profile records in fmt's coverage run, differed from clang's). Found by
// the fmt coverage comparison (WP-941, WP-1006).
//@ expect: run
//@ skip(gcc): the `$_N` spelling is clang's
//@ stdout: N1d3$_0E
#include <cstdio>
#include <typeinfo>
namespace d { enum { e = 1 }; }
int main() { std::puts(typeid(d::e).name()); }
