// [cpp.pragma]: '#pragma weak alias = target' (clang ActOnPragmaWeakAlias)
// defines 'alias' as a weak alias of the extern "C" function 'target'.
// Found while testing PCH prefixes that end with a pending '#pragma weak'
// (WP-1394).
//@ expect: compile-ok
//@ xfail(kiln): WP-1394 follow-up, '#pragma weak name = alias' is rejected as not supported yet
extern "C" int pfx_target() { return 1; }
#pragma weak pfx_alias = pfx_target
