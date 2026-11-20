// [dcl.attr.nouniqueaddr], [intro.object]/9 (WP-1739): a constant-
// initialized object with a potentially-overlapping member of a class
// with tail padding (stored as the class's base-subobject type). The
// member's constant is its complete object's, split into its elements
// only where a later member reuses its tail padding (clang
// `ConstantAggregateBuilder`): a later field, a second such member, a
// derived class's field, a member of union type, a dynamic class (its
// vptr into its own vtable). The run test checks the values.
//@ expect: ir
struct A {
  int x;
  char c;
  constexpr A(int a, char b) : x(a), c(b) {}
};
struct B { [[no_unique_address]] A a; char d; };
B b{{1, 2}, 3};
struct B2 { char d; [[no_unique_address]] A a; };
B2 b2{9, {1, 2}};
struct B3 { [[no_unique_address]] A a; [[no_unique_address]] A b; };
B3 b3{{1, 2}, {3, 4}};
struct B4 { [[no_unique_address]] A a; long l; };
B4 b4{{1, 2}, 5};
struct B5 { [[no_unique_address]] A a; char e; [[no_unique_address]] A b; char f; };
B5 b5{{1, 2}, 3, {4, 5}, 6};
struct C : A {
  char e;
  constexpr C() : A(4, 5), e(6) {}
};
C c;
struct Base { char z; [[no_unique_address]] A a; };
struct Der : Base { char d; };
Der der{{7, {8, 9}}, 10};
union U {
  [[no_unique_address]] A a;
  int i;
  constexpr U() : a(1, 2) {}
};
struct E { [[no_unique_address]] U u; char d; };
E e{{}, 3};
struct V {
  virtual int f() { return 1; }
  char c;
};
struct F { [[no_unique_address]] V v; char d; };
F f{{}, 4};
