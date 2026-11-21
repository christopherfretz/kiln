// [dcl.inline]/6, spec/modules.md §7 "Imported `used` functions"
// (WP-1825): an importer's definitions of the inline functions with GNU
// `used` an imported module unit owns. Each one this unit names is
// defined `linkonce_odr` in a comdat, in `llvm.compiler.used`, right
// before the namespace-scope declaration that names it, with the other
// `used` overloads of the name (named in an unevaluated operand too), as
// clang 22 and 19 emit a declaration they load from the module file; one
// never named is not emitted; a plain inline function waits for the end
// of the unit as before.
//@ module: mod_used_inline=Inputs/mod_used_inline.cppm
import mod_used_inline;
int first() { return 0; }
int calls() { return pick(1) + plain(); }
unsigned long size = sizeof(in_sizeof());
int second() { return 8; }
namespace q {
int nested() { return ns::deep(); }
} // namespace q
void *addr() { return (void *)&one; }
