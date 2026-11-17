// Rejected siblings of conversion-function-derived-result-ok.cpp. With
// 'operator D() const' beside 'operator B&()' clang 22 and GCC 16 find the
// 'B&&' binding not viable, so 'g(B&&)' alone has no viable function (by
// the text 'B&&' binds the 'D' prvalue: a recorded deviation). With
// 'operator D()' beside 'operator B&() const' the function yielding the
// derived class has the better object conversion and binds 'B&&', so
// 'h(B&&)' and 'h(const B&)' are ambiguous, as in the text and both
// compilers.
//@ std: c++11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:3 error "no matching function for call to 'g'"
//@ diag: 21:6 note "candidate function not viable: no known conversion from 'U1' to 'B &&' for 1st argument"
//@ diag: 26:3 error "call to 'h' is ambiguous"
//@ diag: 23:6 note "candidate function"
//@ diag: 22:6 note "candidate function"
struct B { B(); B(const B &); B(B &&); };
struct D : B {};
struct U1 { operator D() const; operator B &(); };
struct U4 { operator D(); operator B &() const; };
void g(B &&);
void h(B &&);
void h(const B &);
void f(U1 u1, U4 u4) {
  g(u1);
  h(u4);
}
