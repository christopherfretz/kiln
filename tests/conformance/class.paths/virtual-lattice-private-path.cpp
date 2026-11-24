// [class.paths]/1, [class.access.base]/5: with every path to L0a ending in a
// private virtual base-specifier (L1a, L1b and L0b all inherit L0a
// privately), no path gives access to `f`, and the note names the
// constraining base-specifier of the first most permissive path. The search
// folds each class once instead of every path (WP-3404).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:20 error "'f' is a private member of 'L0a'"
//@ diag: 12:14 note "constrained by private inheritance here"
//@ diag: 11:19 note "member is declared here"
struct L0a { void f(); }; struct L0b : private virtual L0a {};
struct L1a : private virtual L0a, public virtual L0b {}; struct L1b : private virtual L0a, public virtual L0b {};
struct L2a : private virtual L1a, public virtual L1b {}; struct L2b : private virtual L1a, public virtual L1b {};
struct L3a : private virtual L2a, public virtual L2b {}; struct L3b : private virtual L2a, public virtual L2b {};
struct L4a : private virtual L3a, public virtual L3b {}; struct L4b : private virtual L3a, public virtual L3b {};
struct L5a : private virtual L4a, public virtual L4b {}; struct L5b : private virtual L4a, public virtual L4b {};
struct L6a : private virtual L5a, public virtual L5b {}; struct L6b : private virtual L5a, public virtual L5b {};
void h(L6a& d) { d.f(); }
