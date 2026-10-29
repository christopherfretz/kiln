// [class.abstract]/3, [expr.type.conv]: a temporary of abstract class type cannot
// be created by an explicit type conversion (functional notation, parentheses or
// braces); the note listing pure functions appears once per class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:9 error "allocating an object of abstract class type 'A'"
//@ diag: 12:16 note "unimplemented pure virtual method 'f' in 'A'"
//@ diag: 17:9 error "allocating an object of abstract class type 'A'"
//@ diag: 18:9 error "allocating an object of abstract class type 'B'"
//@ diag: 12:16 note "unimplemented pure virtual method 'f' in 'B'"
struct A {
  virtual void f() = 0;
};
struct B : A {};
void h() {
  (void)A();
  (void)A{};
  (void)B();
}
