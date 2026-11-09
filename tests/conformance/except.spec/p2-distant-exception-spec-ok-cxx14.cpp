// C++14 [except.spec]/2: valid twin of p2-distant-exception-spec-cxx14.cpp.
// A specification on a function declarator, on the function type of a
// top-level pointer, reference or pointer to member, on a parameter of
// (pointer to) function type at any depth, and on the function type a
// returned pointer denotes is allowed. Clang CXX/except/except.spec/
// p2-places.cpp, the non-error lines. WP-1102.
//@ std: c++14
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-c++17-compat-mangling

struct S;
void f() throw(int);
void fn() noexcept(false);
void (*fp)() throw(int);
void (*fq)() noexcept;
void fnx() noexcept;
void (&fr)() noexcept = fnx;
void (S::*mp)() noexcept;
void g(void pfa() throw(int));
void gp(void (*pfa)() noexcept);
void gq(void (*)() noexcept, void (&)() throw());
void (*h())() throw(int);
void (*hn())() noexcept;
void (*i() throw(int))(void (*)() throw(float)) throw(double);
void (**k)(void pfa() throw(int));
void (**kn)(void (*)() noexcept);
auto lam = [](void (*p)() noexcept) { return p; };
void use() {
  void (*local)() noexcept = nullptr;
  (void)local;
  (void)sizeof(void (*)() noexcept);
  (void)static_cast<void (*)() noexcept>(nullptr);
}
