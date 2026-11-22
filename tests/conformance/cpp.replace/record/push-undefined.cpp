// [cpp.pragma] (`push_macro`/`pop_macro`, an extension): pushing an
// undefined name saves "undefined"; its pop ends the definition made
// meanwhile and restores nothing. A pop with nothing pushed changes
// nothing, with a warning, and its row says so (spec/pp.md
// "Preprocessing record", push and pop).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: push-undefined.record
//@ diag: 18:9 warning "pragma pop_macro could not pop 'U', no matching push_macro"
#pragma push_macro("U")
#define U 1
int a = U;
#pragma pop_macro("U")
#ifdef U
#error U is defined
#endif
int U = 2;
#pragma pop_macro("U")
