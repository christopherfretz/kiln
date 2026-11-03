// [dcl.pre]/P3289R1: the expression of a consteval-block-declaration
// shall be a constant expression; a block that is not is ill-formed
// (an error, never a substitution failure), at every scope and in an
// instantiation.
//@ expect: compile-fail
//@ skip(clang): clang 19 has no consteval blocks (P3289R1)
//@ strict-diags
//@ diag: 24:1 error "consteval block is not a constant expression"
//@ diag: 24:13 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 24:1 note "in call to"
//@ diag: 22:5 note "declared here"
//@ diag: 25:1 error "consteval block is not a constant expression"
//@ diag: 25:15 note "cannot modify an object that is visible outside that expression"
//@ diag: 25:1 note "in call to"
//@ diag: 27:3 error "consteval block is not a constant expression"
//@ diag: 27:21 note "unhandled exception of type 'int' with content 1 thrown from here"
//@ diag: 30:31 error "consteval block is not a constant expression"
//@ diag: 32:8 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 30:62 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 30:31 note "in call to"
//@ diag: 22:5 note "declared here"
int rt();
int g;
consteval { rt(); }
consteval { g = 1; }
void f() {
  consteval { throw 1; }
  (void)0;
}
template <class T> struct A { consteval { if (sizeof(T) > 1) rt(); } };
A<char> ok;
A<int> bad;
