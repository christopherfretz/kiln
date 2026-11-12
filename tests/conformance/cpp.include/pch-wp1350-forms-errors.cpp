// [dcl.init.aggr]: a header's designated initializer naming no member
// is diagnosed when its template is instantiated in the unit, with the
// designator's location, through a precompiled header (WP-1350).
//@ expect: compile-fail
//@ diag: 7:46 error "field designator 'zz' does not refer to any field in type 'wf::Out'"
#include "Inputs/wp1350_forms.h"
template <class T> T miss(int x) { return T{.zz = x}; }

int main() { return wf::reorder<wf::Out>(1).a + miss<wf::Out>(2).a; }
