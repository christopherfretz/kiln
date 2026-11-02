// [expr.new]/8: a scoped enumeration does not convert to std::size_t.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:23 error "converting 'E' to incompatible type '__size_t' (aka 'unsigned long')"
//@ requires: clang>=22
enum class E { A = 2 };
void f(E e) { new int[e]; }
