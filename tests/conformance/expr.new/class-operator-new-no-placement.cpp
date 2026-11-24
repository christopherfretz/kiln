// [expr.new]/12: a class's own `operator new` is found in the class's
// scope and hides the global placement form, so `new (p) C` has no
// viable allocation function. The "include <new>" hint is only for a
// lookup in the global scope (clang `resolveAllocationOverload`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:19 error "no matching function for call to 'operator new'"
//@ diag: 11:16 note "candidate function not viable: requires 1 argument, but 2 were provided"
using size_t = decltype(sizeof 0);
struct C {
  static void *operator new(size_t);
};
void f(void *p) { new (p) C; }
