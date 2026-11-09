// C++17 [except.spec]/1 and [diff.cpp14.except]: the exception
// specification is part of the function type, so it may appear wherever a
// function type may, including beyond one level of indirection and in a
// typedef or alias (the C++14 forms of p2-distant-exception-spec-cxx14.cpp
// and exception-spec-in-typedef-cxx14.cpp are valid). WP-1102.
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags

struct S;
typedef void (*fp)() noexcept;
typedef void (*fq)() noexcept(false);
using A = void (**)() noexcept;
void (**j)() noexcept;
void (*(*h())())() noexcept;
void (*arr[2])() noexcept;
void (S::**mpp)() noexcept;
void (*S::*pmp)() noexcept;
void p(void (**)() noexcept, void (*q[])() noexcept);
template <class T> struct X {};
X<void (**)() noexcept> xa;
void g() {
  fp loc = nullptr;
  A a = nullptr;
  (void)loc;
  (void)a;
  (void)sizeof(void (**)() noexcept);
  (void)static_cast<void (**)() noexcept>(nullptr);
  (void)new (void (*[1])() noexcept);
}
