// [class.base.init]/13, /16: clang's -Wuninitialized for a member or base used
// by an initializer that runs before its own (a value use; a reference member
// bound; through a default member initializer, of a written or an implicit
// default constructor); assignments, unevaluated operands, addresses of
// trivial members and const-reference arguments are not uses. Matches clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wuninitialized
//@ diag: 33:14 warning "field 'b' is uninitialized when used here"
//@ diag: 34:41 warning "reference 'r' is not yet bound to a value when used here"
//@ diag: 34:47 warning "field 'd' is uninitialized when used here"
//@ diag: 35:27 warning "field 'b' is uninitialized when used here"
//@ diag: 36:21 warning "field 'b' is uninitialized when used here"
//@ diag: 37:16 warning "field 'd' is uninitialized when used here"
//@ diag: 37:24 warning "field 'd' is uninitialized when used here"
//@ diag: 39:15 warning "field 'z' is uninitialized when used here"
//@ diag: 39:25 warning "field 'b' is uninitialized when used here"
//@ diag: 42:31 warning "base class 'B' is uninitialized when used here to access 'B::bx'"
//@ diag: 43:20 warning "field 'b' is uninitialized when used here"
//@ diag: 43:30 note "during field initialization in this constructor"
//@ diag: 44:20 warning "field 'b' is uninitialized when used here"
//@ diag: 45:3 note "in implicit default constructor for 'G' first required here"
//@ diag: 44:8 note "during field initialization in the implicit default constructor"
//@ diag: 46:40 warning "field 'y' is uninitialized when used here"
struct C { int v; int get() const; C(); C(const C&); };
int g(int);
int h(const int&);
struct S {
  int a, b;
  int &r;
  C c, d;
  int z = 3;
  S(int) : a(b), b(1), r(a), c(), d() {}
  S(char) : a(&b - &a), b(sizeof(b)), r(r), c(d), d() {}
  S(short) : a(h(b)), b(g(b)), r(a), c(), d(c) {}
  S(long) : a(this->b + 1), b(a), r(b), c(), d() {}
  S(float) : a(d.v), b(d.get()), r(z), c(), d() {}
  S(double) : a(b = 3), b(++a), r(a), c(), d() {}
  S(bool) : a(z), b(a ? b : 1), r(a), c(), d() {}
};
struct B { int bx; B(int); };
struct E : B { int e; E() : B(bx) {} };
struct F { int a = b; int b; F() {} };
struct G { int a = b; int b; };
G gg;
struct D : B { int x, y; D() : B(1), x(y), y(bx) {} };
