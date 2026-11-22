// [cpp.pragma] (`push_macro`/`pop_macro`, an extension): the expansion
// after `pop_macro` names the first definition again, by the same index
// and key (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: push-pop.record
#define M 1
#pragma push_macro("M")
#undef M
#define M 2
int a = M;
#pragma pop_macro("M")
int b = M;
