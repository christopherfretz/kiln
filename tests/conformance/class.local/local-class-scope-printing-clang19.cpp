// [temp.arg.type], [class.local]: a class local to a function, a class
// nested in an unnamed class, and a closure type, printed canonically in a
// diagnostic (the instantiation note and the static_assert requirement).
// clang 19 prints no enclosing function and skips an unnamed enclosing
// class. The clang 22 shape is local-class-scope-printing.cpp
// (WP-2477, WP-2545).
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(Local) == 0'"
//@ diag: 42:16 note "in instantiation of template class 'store<Local>' requested here"
//@ diag: 38:59 note "expression evaluates to '1 == 0'"
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(Local::In) == 0'"
//@ diag: 43:20 note "in instantiation of template class 'store<Local::In>' requested here"
//@ diag: 38:59 note "expression evaluates to '1 == 0'"
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(Local::E) == 0'"
//@ diag: 44:19 note "in instantiation of template class 'store<Local::E>' requested here"
//@ diag: 38:59 note "expression evaluates to '4 == 0'"
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(O::In) == 0'"
//@ diag: 47:27 note "in instantiation of template class 'store<O::In>' requested here"
//@ diag: 38:59 note "expression evaluates to '1 == 0'"
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(L::In) == 0'"
//@ diag: 48:65 note "in instantiation of template class 'store<L::In>' requested here"
//@ diag: 38:59 note "expression evaluates to '1 == 0'"
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(L::In) == 0'"
//@ diag: 49:82 note "in instantiation of template class 'store<L::In>' requested here"
//@ diag: 50:15 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 38:59 note "expression evaluates to '1 == 0'"
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(L::In) == 0'"
//@ diag: 51:72 note "in instantiation of template class 'store<L::In>' requested here"
//@ diag: 38:59 note "expression evaluates to '1 == 0'"
//@ diag: 38:49 error "static assertion failed due to requirement 'sizeof(L::In) == 0'"
//@ diag: 52:79 note "in instantiation of template class 'store<L::In>' requested here"
//@ diag: 38:59 note "expression evaluates to '1 == 0'"
//@ diag: 53:45 error "static assertion failed due to requirement 'sizeof((lambda at tests/conformance/class.local/local-class-scope-printing-clang19.cpp:54:21)) > 4'"
//@ diag: 54:39 note "in instantiation of function template specialization 'r<(lambda at tests/conformance/class.local/local-class-scope-printing-clang19.cpp:54:21)>' requested here"
//@ diag: 53:55 note "expression evaluates to '1 > 4'"
template <class T> struct store { static_assert(sizeof(T) == 0); };
namespace n { void f(int, ...); }
void n::f(int, ...) {
  struct Local { struct In {}; enum E { e }; };
  store<Local> a;
  store<Local::In> b;
  store<Local::E> c;
}
struct O { struct { struct In {}; } u; };
store<decltype(O::u)::In> d;
namespace { void g() { struct L { struct In {}; }; store<L::In> e; } }
template <class T> void h(T) { struct L { struct In {}; }; store<typename L::In> f; }
template void h<int>(int);
struct C { void m() const & { struct L { struct In {}; }; store<L::In> g; } };
void p(int (*)(char), const int &) { struct L { struct In {}; }; store<L::In> i; }
template <class T> void r() { static_assert(sizeof(T) > 4); }
void q() { auto l = [] { return 0; }; r<decltype(l)>(); }
