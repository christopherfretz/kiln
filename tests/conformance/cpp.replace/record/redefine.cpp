// [cpp.scope], [cpp.replace]: a definition ended by `#undef` and its
// successor, each expansion naming its own definition; then a definition
// replaced by an identical redefinition (spec/pp.md "Preprocessing
// record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: redefine.record
int a, b;
#define V a
int x = V;
#undef V
#define V b
int y = V;
#define W a
#define W a
int z = W;
