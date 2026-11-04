// [cpp.predefined]/4, [cpp.replace]: the driver's -D defines a macro before
// the first token as `#define FOO 1`; `-Wp,<list>` (GCC's preprocessor-
// option passthrough, used by hardening builds as -Wp,-D_FORTIFY_SOURCE=2)
// splits <list> at commas and hands each piece to the preprocessor, so
// -Wp,-DFOO=1,-UBAR,-DBAZ defines FOO as 1, undefines BAR and defines BAZ
// as 1 (spec/driver.md "Codegen flags").
//@ args: -DBAR -Wp,-DFOO=1,-UBAR,-DBAZ
//@ expect: run
//@ strict-diags
#ifndef FOO
#error FOO not defined
#endif
#ifdef BAR
#error BAR still defined
#endif
int main() { return FOO == 1 && BAZ == 1 ? 0 : 1; }
