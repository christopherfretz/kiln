// The invalid twin of conversion-template-dependent-ordering.cpp: two
// conversion function templates that are equally specialized make the
// explicit instantiation, the explicit specialization and the call
// ambiguous; a non-dependent conversion-function-id is still looked up in
// the template definition. WP-956.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:14 error "partial ordering for explicit instantiation of 'operator char *' is ambiguous"
//@ diag: 20:25 note "explicit instantiation candidate function 'X1::operator char *<char>' template here [with T = char]"
//@ diag: 21:43 note "explicit instantiation candidate function 'X1::operator char *<char, int>' template here [with T = char, U = int]"
//@ diag: 24:28 error "call to member function 'operator long *' is ambiguous"
//@ diag: 20:25 note "candidate function [with T = long]"
//@ diag: 21:43 note "candidate function [with T = long, U = int]"
//@ diag: 26:47 error "no member named 'operator double' in 'Foo'"
//@ diag: 28:17 error "function template specialization 'operator short *' ambiguously refers to more than one function template; explicitly specify template arguments to identify a particular function template"
//@ diag: 20:25 note "function template 'X1::operator short *<short>' matches specialization [with T = short]"
//@ diag: 21:43 note "function template 'X1::operator short *<short, int>' matches specialization [with T = short, U = int]"
struct X1 {
  template <typename T> operator T *() const;
  template <typename T, typename U = int> operator T *() const;
};
template X1::operator char *() const;
void call(const X1 &x) { x.operator long *(); }
struct Foo {
  template <typename T> T As() { return this->operator double(); }
};
template <> X1::operator short *() const;
