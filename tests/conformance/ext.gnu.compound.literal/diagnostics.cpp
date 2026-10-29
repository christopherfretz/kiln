// Compound literal errors: the initialization is a C-style cast's, so a
// reference cannot bind to the list and a scalar takes one element.
//@ expect: compile-fail
//@ args: -Wno-literal-conversion -Wno-constant-conversion
//@ diag: 19:9 error "variable has incomplete type 'void'"
//@ diag: 20:9 error "variable has incomplete type 'Inc'"
//@ diag: 17:8 note "forward declaration of 'Inc'"
//@ diag: 21:9 error "reference to type 'int' cannot bind to an initializer list"
//@ diag: 22:9 error "reference to type 'const int' cannot bind to an initializer list"
//@ diag: 23:9 error "function-style cast to a builtin type can only take one argument"
//@ diag: 24:9 error "function-style cast to a builtin type can only take one argument"
//@ diag: 25:15 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 25:15 note "insert an explicit cast to silence this issue"
//@ diag: 26:15 warning "too many braces around scalar initializer"
//@ diag: 27:19 error "constant expression evaluates to 300 which cannot be narrowed to type 'char'"
//@ diag: 27:19 note "insert an explicit cast to silence this issue"
struct Inc;
void f() {
  (void)(void){};
  (void)(Inc){1};
  (void)(int&){1};
  (void)(const int&){1};
  (void)(int){1, 2};
  (void)(int){{1}, 2};
  (void)(int){1.5};
  (void)(int){{1}};
  char c = (char){300};
  (void)c;
}
