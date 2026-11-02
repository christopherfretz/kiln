// [namespace.def.general]/3: `inline` shall appear on the original
// definition of an inline namespace; an extension definition may omit it
// (the namespace stays inline), but clang warns
// (-Winline-namespace-reopened-noninline) with a note at the original
// definition, also through a nested namespace and when the extension
// finds the namespace through the enclosing namespace's inline set.
//@ strict-diags
//@ diag: 13:11 warning "inline namespace reopened as a non-inline namespace"
//@ diag: 12:18 note "previous definition is here"
//@ diag: 15:25 warning "inline namespace reopened as a non-inline namespace"
//@ diag: 14:32 note "previous definition is here"
inline namespace A { int a; }
namespace A { int b; }
namespace O { inline namespace I { int e; } }
namespace O { namespace I { int f; } }
namespace O { inline namespace I { int h; } }
int g = A::b + b + O::f + O::e + O::h;
