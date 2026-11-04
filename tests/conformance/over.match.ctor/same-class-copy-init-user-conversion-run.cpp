// [over.match.copy]/1, [over.best.ics]/4: copy-initializing `A` from an
// lvalue of a class derived from `A` goes through `A`'s constructors
// ([dcl.init.general]/16.6.2), and their parameters may be initialized by a
// user-defined conversion of the argument: `const B` cannot bind `A&`, so
// `A(X)` is selected through `B::operator X()`. Only the copy-initialization
// from an unrelated class suppresses user-defined conversions on the
// constructor's parameter. Matches clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: X X ref
extern "C" int printf(const char*, ...);
struct X {};
struct A {
  A() {}
  A(A&) { printf("ref"); }
  A(X) { printf("X "); }
};
struct B : A {
  operator X() const { return {}; }
};
int main() {
  const B cb;
  A a = cb;
  A a2(cb);
  B b;
  A a3 = b;
  printf("\n");
  return 0;
}
