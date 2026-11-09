// [class.mem.general]/5: a member is declared once in the
// member-specification, except that a nested class can be declared and
// later defined. A member class, union or enumeration redeclared without a
// definition is clang's extension warning (-Wredeclared-class-member, an
// error under -pedantic-errors), with a note at the latest declaration; in a
// class template it is reported once, at the template definition.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ diag: 27:10 warning "class member cannot be redeclared"
//@ diag: 26:10 note "previous declaration is here"
//@ diag: 30:9 warning "class member cannot be redeclared"
//@ diag: 29:9 note "previous declaration is here"
//@ diag: 33:8 warning "class member cannot be redeclared"
//@ diag: 32:8 note "previous declaration is here"
//@ diag: 35:14 warning "class member cannot be redeclared"
//@ diag: 34:14 note "previous declaration is here"
//@ diag: 37:10 warning "class member cannot be redeclared"
//@ diag: 36:10 note "previous declaration is here"
//@ diag: 38:10 warning "class member cannot be redeclared"
//@ diag: 37:10 note "previous declaration is here"
//@ diag: 42:10 warning "class member cannot be redeclared"
//@ diag: 41:10 note "previous declaration is here"
struct A {
  struct B;
  struct B {};
  struct B;
  union U;
  union U {};
  union U;
  enum E1 : int;
  enum E1 : int { e1 };
  enum E1 : int;
  enum class E2;
  enum class E2;
  struct F;
  struct F;
  struct F;
};
template <typename T> struct C {
  struct B {};
  struct B;
};
C<int> c;
