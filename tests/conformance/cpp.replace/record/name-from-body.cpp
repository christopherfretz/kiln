// [cpp.rescan]: a function-like macro named by another's replacement list
// and invoked with an argument from the file: `F`'s name is in `APPLY`'s
// body, its end and argument span in the file (spec/pp.md "Preprocessing
// record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: name-from-body.record
namespace L { int v; }
#define F(x) x
#define APPLY F
int r = APPLY(L::v);
