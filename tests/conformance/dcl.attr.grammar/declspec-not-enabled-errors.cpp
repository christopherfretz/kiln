// `__declspec(...)` is a Microsoft extension that clang enables only
// with `-fdeclspec` or `-fms-extensions` (not offered here): clang's
// "'__declspec' attributes are not enabled" at the keyword, the
// parenthesized list skipped and the declaration kept (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:1 error "'__declspec' attributes are not enabled; use '-fdeclspec' or '-fms-extensions' to enable support for __declspec attributes"
//@ diag: 11:1 error "'__declspec' attributes are not enabled; use '-fdeclspec' or '-fms-extensions' to enable support for __declspec attributes"

__declspec(noinline) void f();
__declspec(align(16)) int x;
void g() { f(); (void)x; }
