// [temp.names]/3, [temp.inst], [dcl.enum], [temp.constr]: through a
// precompiled header, an instantiation through a qualified template name
// still fails, std::align_val_t is still defined, and a constraint built
// from a qualified template name is still checked (WP-1353).
//@ expect: compile-fail
//@ diag: 10:28 error "redefinition of 'align_val_t'"
//@ diag: 13:9 error "no matching function for call to 'need'"
//@ diag: 12:25 note "constraints not satisfied [with T = char]"
#include "Inputs/decl_followups.h"
namespace std { enum class align_val_t : size_t {}; }
f2::wrap<void> w;
template <f2::pi T> int need(T) { return 0; }
int x = need('c');
