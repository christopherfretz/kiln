// __builtin_addressof (spec/sema-templates.md §9.5): one argument, which
// must be an addressable lvalue.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:30 error "too few arguments to function call, expected 1, have 0"
//@ diag: 12:33 error "too many arguments to function call, expected 1, have 2"
//@ diag: 14:10 error "address of bit-field requested"
//@ diag: 16:10 error "cannot take the address of an rvalue of type 'int'"
//@ diag: 17:10 error "cannot take the address of an rvalue of type 'int'"
int g;
int *a = __builtin_addressof();
int *b = __builtin_addressof(g, g);
struct B { int f : 3; } bb;
auto c = __builtin_addressof(bb.f);
int &&h();
auto e = __builtin_addressof(h());
auto k = __builtin_addressof(1);
