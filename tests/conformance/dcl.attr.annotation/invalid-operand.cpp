//@ expect: compile-fail
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ strict-diags
//@ diag: 18:3 error "annotation is not a constant expression"
//@ diag: 18:4 note "read of non-const variable 'runtime'"
//@ diag: 17:5 note "declared here"
//@ diag: 20:3 error "annotation has non-structural type 'Mutable'"
//@ diag: 19:30 note "mutable non-static data member"
//@ diag: 21:3 error "annotation has non-structural type 'void'"
//@ diag: 23:5 error "unexpanded parameter pack"
//@ diag: 25:23 error "cannot be used prior to '::'"
//@ diag: 29:3 note "in instantiation of annotation for 'f<char>' requested here"
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
