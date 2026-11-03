//@ expect: compile-fail
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ strict-diags
//@ diag: 20:3 error "annotation is not a constant expression"
//@ diag: 20:4 note "read of non-const variable 'runtime'"
//@ diag: 19:5 note "declared here"
//@ diag: 22:3 error "annotation has non-structural type 'Mutable'"
//@ diag: 21:30 note "mutable non-static data member"
//@ diag: 23:3 error "annotation has non-structural type 'void'"
//@ diag: 25:5 error "unexpanded parameter pack"
//@ diag: 27:23 error "cannot be used prior to '::'"
//@ diag: 31:3 note "in instantiation of annotation for 'f<char>' requested here"
//@ diag: 34:30 error "cannot be used prior to '::'"
//@ diag: 38:3 note "in instantiation of annotation for 'g<char>' requested here"
// [dcl.attr.annotation]/2: `std::meta::reflect_constant(e)` must be a
// constant expression, so the operand is a constant expression of a
// structural type. [dcl.attr.annotation]/4: substitution into an
// annotation is not in the immediate context (P3394R4 Example 4).
int runtime = 1;
[[=runtime]] int non_constant;
struct Mutable { mutable int m; };
[[=Mutable{}]] int not_structural;
[[=(void)0]] int void_operand;
template <class... Ts> struct Pack {
  [[=sizeof(Ts)]] int unexpanded;
};
template <class T> [[=T::type()]] void f(T);
void f(int);
void use() {
  f(0);
  f('0');
}
// The same for a parameter's annotation.
template <class T> void g([[=T::type()]] T);
void g(int);
void use_param() {
  g(0);
  g('0');
}
