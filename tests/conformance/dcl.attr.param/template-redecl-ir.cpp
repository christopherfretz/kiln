// [temp.inst]/3, [dcl.attr.grammar]/5: a function template
// specialization's parameters carry the attributes of the parameters of
// the template's latest declaration where the specialization is
// declared, their own and the ones they inherited: `nonnull` written only
// on the definition or only on a later declaration reaches it, as does
// `align_value`, `annotate`, `maybe_undef` and `noescape` on the
// definition; a first declaration's `align_value` and `maybe_undef`
// (not inherited) do not. A declaration after that point, an explicit
// specialization, a friend declaration and a class template member's
// out-of-class definition do not change it (clang 19 and 22).
// spec/sema.md §4.5 "Parameter attributes" rule 5, WP-3407.
//@ expect: ir
//@ std: c++20
#include "Inputs/template-redecl.h"
