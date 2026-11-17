// [expr.delete]/7 (C++26, P3068R6), rejected side: after a `delete` whose
// destructor threw, the storage is released, so a later use of the
// pointer is a use after free and a second `delete` a double delete; a
// real leak is still reported; an uncaught exception from the destructor
// fails the evaluation; a second element destructor throwing while the
// first exception unwinds terminates ([except.terminate]/1.4).
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:10 note "read of heap allocated object that has been deleted"
//@ diag: 34:15 note "in call to 'use_after()'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:9 note "delete of pointer that has already been deleted"
//@ diag: 41:15 note "in call to 'twice()'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:10 note "allocation performed here was not deallocated"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:42 note "unhandled exception of type 'int' with content 1 thrown from here"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:36 note "exception thrown out of an object destructor while unrolling another exception"
//@ diag: 56:9 note "in call to '{*new T[2]#0}[0].~T()'"
//@ diag: 59:15 note "in call to 'both()'"
//@ diag: 27:36 note "currently active exception thrown here"
struct T {
  int v = 1;
  constexpr ~T() noexcept(false) { throw v; }
};
constexpr int use_after() {
  T *p = new T;
  try { delete p; } catch (int) {}
  return p->v;
}
static_assert(use_after() == 1);
constexpr int twice() {
  T *p = new T;
  try { delete p; } catch (int) {}
  try { delete p; } catch (int) {}
  return 1;
}
static_assert(twice() == 1);
constexpr int leak() {
  T *p = new T;
  T *q = new T;
  try { delete p; } catch (int) {}
  return q != nullptr;
}
static_assert(leak() == 1);
constexpr int uncaught() {
  delete new T;
  return 1;
}
static_assert(uncaught() == 1);
constexpr int both() {
  T *p = new T[2];
  try { delete[] p; } catch (int) {}
  return 1;
}
static_assert(both() == 1);
