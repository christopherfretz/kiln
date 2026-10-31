// [over.call.object]: a closure object is called through its call operator
// and its conversion to pointer to function (a surrogate candidate).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:3 error "no matching function for call to object of type '(lambda at "
//@ diag: 9:3 note "candidate function not viable: requires 0 arguments, but 1 was provided"
//@ diag: 9:3 note "conversion candidate of type 'auto (*)() -> void'"
void f() {
  []{}(1);
}
