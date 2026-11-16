// [class.copy.elision], CWG2327, [expr.const]: constant evaluation elides the
// copy of a temporary made by a user-defined conversion for a copy
// constructor exactly as code generation does (clang's constant evaluator
// does too), so a counting copy constructor sees no copy in a constexpr
// variable, a local, or a new-expression. Found running GCC's C++
// testsuite. Matches clang 19 (GCC 16.1 elides the base-class copy too).
//@ std: c++20
//@ strict-diags
struct B {
  int copies = 0;
  constexpr B(...) {}
  constexpr B(const B& o) : copies(o.copies + 1) {}
};
constexpr B b(1);
static_assert(b.copies == 0);
constexpr int local() {
  B x(2);
  B* p = new B(3);
  int r = x.copies * 10 + p->copies;
  delete p;
  return r;
}
static_assert(local() == 0);
struct D : B {
  constexpr D() : B(1) {}
};
// A base-class initializer is not a complete object: the copy is made.
static_assert(D().copies == 1);
