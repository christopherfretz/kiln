// [cpp.include], [cpp.scope]: an X-macro file reduced from real code,
// included twice under two definitions of `K`. Each inclusion expands
// `K(Add)` and `K(Sub)` at the same offsets, naming a different
// definition, and each `#undef` ends its own (spec/pp.md "Preprocessing
// record"; each inclusion is its own instance, `in i1` and `in i2`).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: x-macro.record
enum Op {
#define K(n) n,
#include "Inputs/k.def"
};
int f(Op o) {
  switch (o) {
#define K(n) case n:
#include "Inputs/k.def"
    return 1;
  }
  return 0;
}
