// [expr.new]/8: a scoped enumeration does not convert to std::size_t.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:23 error "converting 'E' to incompatible type 'unsigned long'"
enum class E { A = 2 };
void f(E e) { new int[e]; }
