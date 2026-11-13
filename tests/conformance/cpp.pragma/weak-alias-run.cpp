// [cpp.pragma]: '#pragma weak alias = target' declares 'alias' at
// namespace scope with the target's type and makes it a weak alias of it
// (clang ActOnPragmaWeakAlias, DeclClonePragmaWeak): for a function or a
// variable declared before the pragma, and for a target declared after
// it. The alias of a function is a C++ function (its name is mangled);
// the alias of a variable is not. WP-1399.
//@ expect: run
//@ stdout: 3 4 9
#include <cstdio>
extern "C" int tgt_fn() { return 3; }
#pragma weak alias_fn = tgt_fn
#pragma weak alias_late = late_fn
extern "C" int late_fn() { return 4; }
extern "C" int tgt_var = 9;
#pragma weak alias_var = tgt_var
int main() { std::printf("%d %d %d\n", alias_fn(), alias_late(), alias_var); }
