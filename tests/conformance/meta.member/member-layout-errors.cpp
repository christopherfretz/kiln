// GCC's member-layout builtins ([meta.member]/2, /4) take pointers to
// members of complete classes. Found running GCC's C++ testsuite (WP-1472).
//@ skip(clang): clang 19/22 do not have the builtins
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 15:60 error "argument to '__builtin_is_pointer_interconvertible_with_class' is not a pointer to member (was 'int *')"
//@ diag: 16:50 error "too few arguments to function call, expected 2, have 1"
//@ diag: 17:60 error "incomplete type 'I' where a complete type is required"
//@ diag: 13:8 note "forward declaration of 'I'"

struct A { int a; };
struct I;
int *p;
bool e1 = __builtin_is_pointer_interconvertible_with_class(p);
bool e2 = __builtin_is_corresponding_member(&A::a);
bool e3 = __builtin_is_pointer_interconvertible_with_class((int I::*)nullptr);
