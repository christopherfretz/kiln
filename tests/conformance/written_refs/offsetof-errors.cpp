// The written-reference table (spec/ast.md §18.2, §18.4, WP-2471): an
// offsetof member-designator ([support.types.layout]/1) whose component
// fails names nothing from there on: a member lookup does not find, the
// members after it, those after a bit-field ([support.types.layout]/1
// excludes them) or after a subscript of a non-array, and every member
// of a designator whose operand is not a type are `(unresolved)`.
//@ expect: compile-fail
//@ written-refs: offsetof-errors.refs
//@ diag: 23:20 error "no member named 'zz' in 'M'"
//@ diag: 24:49 error "cannot compute offset of bit-field 'bits'"
//@ diag: 15:7 note "bit-field is declared here"
//@ diag: 25:45 error "offsetof requires array type, 'int' invalid"
//@ diag: 26:39 error "unknown type name 'Undeclared'"
struct In {
  int bits : 3;
  int c;
};
struct M {
  int x;
  In b[2];
};
struct S { int q; M a; };
unsigned long o1 = __builtin_offsetof(S, a.zz.c);
unsigned long o2 = __builtin_offsetof(S, a.b[1].bits.c);
unsigned long o3 = __builtin_offsetof(S, q[1].c);
unsigned long o4 = __builtin_offsetof(Undeclared, x.y);
