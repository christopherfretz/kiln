// C++14 [except.spec]/2 (removed in C++17, [diff.cpp14.except]): an
// exception-specification shall not appear in a typedef declaration or
// alias-declaration (clang `err_exception_spec_in_typedef`, at the
// `throw`/`noexcept`; the type keeps it). One error per function
// declarator of the typedef, none for a parameter's own declarator or a
// variable. kiln accepted these in C++11/14.
//@ verify
//@ std: c++11

typedef void (*fp1)() throw(); // expected-error {{exception specifications are not allowed in typedefs}}
typedef void (*fp2)() noexcept; // expected-error {{exception specifications are not allowed in typedefs}}
typedef void fn3() noexcept(true); // expected-error {{exception specifications are not allowed in typedefs}}
using fp4 = void (*)() throw(); // expected-error {{exception specifications are not allowed in type aliases}}
using fp5 = void (&)() noexcept; // expected-error {{exception specifications are not allowed in type aliases}}
template <class T> using fp6 = void (*)(T) noexcept; // expected-error {{exception specifications are not allowed in type aliases}}
struct S {
  typedef void (*m1)() noexcept; // expected-error {{exception specifications are not allowed in typedefs}}
  using m2 = void (S::*)() throw(); // expected-error {{exception specifications are not allowed in type aliases}}
};
void g() {
  typedef void (*l1)() noexcept; // expected-error {{exception specifications are not allowed in typedefs}}
  l1 p = nullptr;
  (void)p;
}
typedef void (*(*fp7)() noexcept)() throw(); // expected-error 2 {{exception specifications are not allowed in typedefs}}
void (*ok1)() noexcept;
typedef void (*fp8)(void (*)() noexcept);
