// [over.ics.list]/8, [dcl.init.aggr]/3.1: a designated-initializer-list that
// initializes a subobject twice (the same member, a member of an object an
// earlier clause initialized, another member of the same union, through
// anonymous members too) converts to no aggregate in overload resolution,
// so `f({.a = 1, .b = 2})` picks the struct over the union. Order is not
// checked there. As in clang, the designators of a braced value are not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 53:22 error "no matching function for call to 'h'"
//@ diag: 52:5 note "candidate function not viable: cannot convert initializer list argument to 'S2'"
//@ diag: 52:17 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 55:25 warning "nested designators are a C99 extension"
//@ diag: 55:22 error "no matching function for call to 'n'"
//@ diag: 54:5 note "candidate function not viable: cannot convert initializer list argument to 'N'"
//@ diag: 54:16 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 56:36 warning "nested designators are a C99 extension"
//@ diag: 56:22 error "no matching function for call to 'n'"
//@ diag: 54:5 note "candidate function not viable: cannot convert initializer list argument to 'N'"
//@ diag: 54:16 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 57:25 warning "nested designators are a C99 extension"
//@ diag: 57:22 error "no matching function for call to 'n'"
//@ diag: 54:5 note "candidate function not viable: cannot convert initializer list argument to 'N'"
//@ diag: 54:16 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 58:44 warning "initializer overrides prior initialization of this subobject"
//@ diag: 58:36 note "previous initialization is here"
//@ diag: 59:40 warning "nested designators are a C99 extension"
//@ diag: 59:22 error "no matching function for call to 'n'"
//@ diag: 54:5 note "candidate function not viable: cannot convert initializer list argument to 'N'"
//@ diag: 54:16 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 60:25 warning "nested designators are a C99 extension"
//@ diag: 60:49 warning "array designators are a C99 extension"
//@ diag: 61:25 warning "nested designators are a C99 extension"
//@ diag: 61:29 warning "array designators are a C99 extension"
//@ diag: 61:22 error "no matching function for call to 'n'"
//@ diag: 54:5 note "candidate function not viable: cannot convert initializer list argument to 'N'"
//@ diag: 54:16 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 63:25 warning "nested designators are a C99 extension"
//@ diag: 63:22 error "no matching function for call to 'w'"
//@ diag: 62:5 note "candidate function not viable: cannot convert initializer list argument to 'W'"
//@ diag: 62:16 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 64:25 warning "nested designators are a C99 extension"
//@ diag: 66:22 error "no matching function for call to 'a'"
//@ diag: 65:5 note "candidate function not viable: cannot convert initializer list argument to 'A'"
//@ diag: 65:16 note "candidate function not viable: cannot convert initializer list argument to 'long'"
//@ diag: 72:44 warning "initializer overrides prior initialization of this subobject"
//@ diag: 72:36 note "previous initialization is here"
struct S2 { int a, b; };
struct N { S2 s; int c; int arr[3]; };
union U2 { int a, b; };
struct W { U2 u; int c; };
struct A { union { int x; int y; }; int z; };
int h(S2); char h(long);
static_assert(sizeof(h({.a = 1, .a = 2})) == 4);
int n(N); char n(long);
static_assert(sizeof(n({.s.a = 1, .s.a = 2})) == 4);
static_assert(sizeof(n({.s = S2{}, .s.a = 2})) == 4);
static_assert(sizeof(n({.s.a = 1, .s = S2{}})) == 4);
static_assert(sizeof(n({.s = {.a = 1, .a = 2}})) == 4);
static_assert(sizeof(n({.s = {.a = 1}, .s.b = 2})) == 4);
static_assert(sizeof(n({.s.a = 1, .s.b = 2, .arr[1] = 1, .arr[2] = 2})) == 4);
static_assert(sizeof(n({.arr[1] = 1, .arr[1] = 2})) == 4);
int w(W); char w(long);
static_assert(sizeof(w({.u.a = 1, .u.b = 2})) == 4);
static_assert(sizeof(w({.u.a = 1, .c = 2})) == 4);
int a(A); char a(long);
static_assert(sizeof(a({.x = 1, .y = 2})) == 4);
static_assert(sizeof(a({.x = 1, .z = 2})) == 4);
union U { int a, b; };
struct S { int a, b; };
int f(U); char f(S);
static_assert(sizeof(f({.a = 1, .b = 2})) == 1);
static_assert(sizeof(n({.s = {.a = 1, .a = 2}})) == 4);
