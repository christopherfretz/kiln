// [dcl.inline]/6, spec/modules.md §7 "Imported `used` functions"
// (WP-1832): the `used` inline functions an importer reaches only through
// the body of an imported function it odr-uses. clang 22 and 19 read
// that body when the importer odr-uses the function and load every
// declaration it names in the order the module file stores it (each
// statement's parts last to first, an operand before its declaration),
// so those `used` functions are defined right before the declaration
// that odr-used the function, or at the end of the unit when the use is
// in an instantiation done there. One level only: `inner`'s body is read
// when codegen reaches it, which emits `b15` in it; a local variable's
// initializer (`b4`), a lambda body (`b11`) and a default argument
// (`b10`) are read the same way, later. A body named only in an
// unevaluated operand (`unevaluated`) or a template never instantiated
// is not read. Reduced from real code.
//@ module: mod_used_inline_body=Inputs/mod_used_inline_body.cppm
import mod_used_inline_body;
int z0() { return 0; }
int a() { return order(); }
int z1() { return 0; }
int b() { return branch(); }
int z2() { return 0; }
int c() { return loops(1); }
int z3() { return 0; }
int d() { return defarg(); }
int z4() { return 0; }
int e() { return lam(); }
unsigned long size = sizeof(unevaluated());
int z5() { return 0; }
template <class T> int never() { return tmpl(); }
template <class T> int t() { return tmpl(); }
int z6() { return 0; }
int use = t<int>();
static_assert(ce() == 21);
int z7() { return 0; }
template <class T> struct C { int m = ce2(); };
int z8() { return C<int>().m; }
int f() { return twice(); }
