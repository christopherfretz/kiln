// [stmt.return]/6 (P2748R5), [expr.cast]/4, [expr.static.cast]/4: a C-style
// or functional cast to a reference type that binds a temporary
// (`(const int&)c` for a `char c`) is a temporary bound by the returned
// reference, also through a template's `T(e)` with `T` a reference type (as
// libc++'s `make_from_tuple` does). clang points at the temporary. Verified
// against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:46 error "returning reference to local temporary object"
//@ diag: 21:38 error "returning reference to local temporary object"
//@ diag: 22:56 error "returning reference to local temporary object"
//@ diag: 18:50 error "returning reference to local temporary object"
//@ diag: 26:9 note "in instantiation of function template specialization 'f<const int &, char>' requested here"
//@ diag: 19:43 error "returning reference to local temporary object"
//@ diag: 27:9 note "in instantiation of function template specialization 'f3<const int &>' requested here"
template<class T> T&& fwd(T& t) { return static_cast<T&&>(t); }
using CIR = const int&;
template<class T, class U> T f(U&& u) { return T(fwd<U>(u)); }
template<class T> T f3(char c) { return T(c); }
const int& h0(char&& c) { return (const int&)(static_cast<char&&>(c)); }
const int& h1(char&& c) { return CIR(static_cast<char&&>(c)); }
const int& h2(char c) { return static_cast<const int&>(c); }
const char& ok(char& c) { return (const char&)c; }
void g() {
  char c = 0;
  (void)f<const int&>(static_cast<char&&>(c));
  (void)f3<const int&>(c);
}
