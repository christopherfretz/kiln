// [basic.start.dynamic]/1, [intro.execution]/7: a namespace-scope
// variable with internal linkage that nothing uses is still dynamically
// initialized when its initializer reads a volatile object, since the
// read is a side effect. kiln dropped `anchor_dest` and `local_dest`
// with their initializers, so LLVM's anchor idiom
// (`static int JSONAnchorDest = JSONAnchorSource;`) no longer pulled
// JSONCompilationDatabase.o out of libclangTooling.a and clang-check /
// c-index-test found no JSON compilation database. A non-volatile read
// is not a side effect, and that variable is not emitted.
//@ expect: ir
//@ requires: clang>=22
extern volatile int anchor_source;
static int __attribute__((unused)) anchor_dest = anchor_source;

volatile int local_source = 3;
static int local_dest = local_source;

extern int plain_source;
static int plain_dest = plain_source;
