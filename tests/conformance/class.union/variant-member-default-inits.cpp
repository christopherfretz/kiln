// [class.union.general]/6 (CWG1460): at most one variant member of a union
// has a default member initializer, counting the members of its anonymous
// members; every later one is diagnosed at its name with a note at the
// first (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:38 error "initializing multiple members of union"
//@ diag: 30:24 note "previous initialization is here"
//@ diag: 31:35 error "initializing multiple members of union"
//@ diag: 31:16 note "previous initialization is here"
//@ diag: 32:50 error "initializing multiple members of union"
//@ diag: 32:25 note "previous initialization is here"
//@ diag: 33:50 error "initializing multiple members of union"
//@ diag: 33:33 note "previous initialization is here"
//@ diag: 34:46 error "initializing multiple members of union"
//@ diag: 34:24 note "previous initialization is here"
//@ diag: 35:41 error "initializing multiple members of union"
//@ diag: 35:23 note "previous initialization is here"
//@ diag: 35:52 error "initializing multiple members of union"
//@ diag: 35:23 note "previous initialization is here"
//@ diag: 36:43 error "initializing multiple members of union"
//@ diag: 36:16 note "previous initialization is here"
//@ diag: 37:41 error "initializing multiple members of union"
//@ diag: 37:32 note "previous initialization is here"
//@ diag: 38:26 error "initializing multiple members of union"
//@ diag: 38:16 note "previous initialization is here"
//@ diag: 39:37 error "initializing multiple members of union"
//@ diag: 39:26 note "previous initialization is here"
union U1 { union { int a = 1; }; int b = 2; };
union U2 { int b = 2; union { int a = 1; }; };
union U3 { struct { int a = 1; int c = 2; }; int b = 3; };
union U4 { struct { union { int a = 1; }; }; int b = 2; };
union U5 { union { int a = 1; }; union { int b = 2; }; };
union U6 { int a; int b = 1; int c; int d = 2; int e = 3; };
union U7 { int a = 1; struct { int b; int c = 2; }; };
template<class T> union U8 { T a = 1; T b = 2; };
union U9 { int a{1}; int b{2}; };
struct S10 { union { int a = 1; int b = 2; }; };
