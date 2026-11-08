// [expr.call]/7 (a parameter is copy-initialized from its argument; an
// argument with no parameter undergoes the default argument promotions),
// [class.temporary]/3: clang 21+ marks an indirectly passed argument
// `dead_on_return` when its lifetime ends in the callee: a trivially
// destructible class or vector passed by reference to a caller-made
// copy, also as a variadic argument, and a `[[clang::trivial_abi]]`
// class with a non-trivial destructor, which the callee destroys
// (`isParamDestroyedInCallee`); a class the caller destroys gets none
// (spec/codegen.md §6a, WP-774). On AArch64 these are the aggregates
// over 16 bytes; x86-64 passes them `byval` (no attribute). Compared
// with clang 22's IR exactly.
//@ expect: ir
// clang 22 has no C++26 erroneous values (P2795R5, spec/codegen.md §7r).
//@ args: -ftrivial-auto-var-init=uninitialized
//@ requires: clang>=22
//@ requires: llvm>=22
typedef int v16 __attribute__((vector_size(64)));
struct Big { long a, b, c, d; };
struct Arr { char c[17]; };
struct [[clang::trivial_abi]] TABig { long a[3]; ~TABig(); };
struct Dtor { long a[3]; ~Dtor(); };
struct Holder { TABig t; };

void vf(int, ...);
void take_t(TABig);
void take_d(Dtor);
void take_h(Holder);

long callee(TABig t, Dtor d, Holder h) { return t.a[0] + d.a[1] + h.t.a[2]; }

void caller(Big b, Arr a, v16 v, TABig t, Dtor d, Holder h) {
  vf(3, b, a, v);
  take_t(t);
  take_d(d);
  take_h(h);
}
