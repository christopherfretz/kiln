// [cpp.include], [cpp.cond]: a `.def` file included twice under two
// definitions of `K`. In each instance `#ifndef K` is not taken and reads
// `K` bound to that instance's definition, at the same offset; it is not
// a guard; `K(Add)` has the same key in both instances and names a
// different definition in each (spec/pp.md "Preprocessing record",
// condition reads).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: def-twice.record
enum E {
#define K(n) n,
#include "Inputs/k.def"
};
int f(E e) {
  switch (e) {
#define K(n) case n:
#include "Inputs/k.def"
    return 1;
  }
  return 0;
}
