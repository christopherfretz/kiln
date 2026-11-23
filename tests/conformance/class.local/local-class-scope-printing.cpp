// [temp.arg.type], [class.local]: a class local to a function, a class
// nested in an unnamed class, and a closure type, printed canonically in a
// diagnostic (the instantiation note and the static_assert requirement).
// clang 22 prints the enclosing function of a class nested in a local
// class, with its parameter types and without its template arguments, and
// an unnamed enclosing class as `(unnamed struct)`; a class local to the
// function itself prints unqualified. The clang 19 shape is
// local-class-scope-printing-clang19.cpp. The key printer that keeps
// these apart is tested in crates/kiln_parse/tests/type_keys.rs
// (WP-2477, WP-2545).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof(Local) == 0'"
//@ diag: 46:16 note "in instantiation of template class 'store<Local>' requested here"
//@ diag: 42:59 note "expression evaluates to '1 == 0'"
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof(n::f(int, ...)::Local::In) == 0'"
//@ diag: 47:20 note "in instantiation of template class 'store<n::f(int, ...)::Local::In>' requested here"
//@ diag: 42:59 note "expression evaluates to '1 == 0'"
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof(n::f(int, ...)::Local::E) == 0'"
//@ diag: 48:19 note "in instantiation of template class 'store<n::f(int, ...)::Local::E>' requested here"
//@ diag: 42:59 note "expression evaluates to '4 == 0'"
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof(O::(unnamed struct)::In) == 0'"
//@ diag: 51:27 note "in instantiation of template class 'store<O::(unnamed struct)::In>' requested here"
//@ diag: 42:59 note "expression evaluates to '1 == 0'"
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof((anonymous namespace)::g()::L::In) == 0'"
//@ diag: 52:65 note "in instantiation of template class 'store<(anonymous namespace)::g()::L::In>' requested here"
//@ diag: 42:59 note "expression evaluates to '1 == 0'"
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof(h(int)::L::In) == 0'"
//@ diag: 53:82 note "in instantiation of template class 'store<h(int)::L::In>' requested here"
//@ diag: 54:15 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 42:59 note "expression evaluates to '1 == 0'"
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof(C::m()::L::In) == 0'"
//@ diag: 55:72 note "in instantiation of template class 'store<C::m()::L::In>' requested here"
//@ diag: 42:59 note "expression evaluates to '1 == 0'"
//@ diag: 42:49 error "static assertion failed due to requirement 'sizeof(p(int (*)(char), const int &)::L::In) == 0'"
//@ diag: 56:79 note "in instantiation of template class 'store<p(int (*)(char), const int &)::L::In>' requested here"
//@ diag: 42:59 note "expression evaluates to '1 == 0'"
//@ diag: 57:45 error "static assertion failed due to requirement 'sizeof((lambda at tests/conformance/class.local/local-class-scope-printing.cpp:58:21)) > 4'"
//@ diag: 58:39 note "in instantiation of function template specialization 'r<(lambda at tests/conformance/class.local/local-class-scope-printing.cpp:58:21)>' requested here"
//@ diag: 57:55 note "expression evaluates to '1 > 4'"
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
