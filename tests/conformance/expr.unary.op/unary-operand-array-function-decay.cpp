// [expr.unary.op]/7-/10: the operand of unary `+` may be a pointer, and
// an array or function operand is converted to one first ([conv.array],
// [conv.func]), so `+fn` and `+a` are pointers; `-` and `~` reject them,
// reported with the pointer type (clang `CreateBuiltinUnaryOp`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:17 error "invalid argument type 'int *' to unary expression"
//@ diag: 15:17 error "invalid argument type 'int *' to unary expression"
//@ diag: 16:17 error "invalid argument type 'void (*)()' to unary expression"
//@ diag: 17:17 error "invalid argument type 'void (*)()' to unary expression"
int a[10];
void fn();
static_assert(sizeof(+a) == sizeof(int *));
int s2 = sizeof(-a);
int s3 = sizeof(~a);
int s6 = sizeof(-fn);
int s7 = sizeof(~fn);
void (*p)() = +fn;
static_assert(sizeof(!fn) == sizeof(bool));
