// [dcl.init.aggr]/3.1: a field designator names a direct non-static data
// member (or one of an anonymous member) of the class being initialized;
// a field designator into a scalar and an array designator into a class are
// errors. A union initialized through two different members is a
// DefaultError -Winitializer-overrides warning.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:11 error "field designator 'q' does not refer to any field in type 'Pt'"
//@ diag: 22:9 warning "nested designators are a C99 extension"
//@ diag: 22:11 error "field designator cannot initialize a non-class type 'int'"
//@ diag: 23:10 warning "array designators are a C99 extension"
//@ diag: 23:10 error "array designator cannot initialize non-array type 'Pt'"
//@ diag: 24:14 error "field designator cannot initialize a non-class type 'int[3]'"
//@ diag: 25:17 error "initializer partially overrides prior initialization of this subobject"
//@ diag: 25:14 note "previous initialization is here"
//@ diag: 26:9 warning "nested designators are a C99 extension"
//@ diag: 26:12 error "field designator 'q' does not refer to any field in type 'Pt'"
struct Pt { int x; int y; int z; };
struct Q { Pt p; int w; };
union U { int i; float f; };
Pt a9 = {.q = 1};
Q b1 = {.w.x = 1};
Pt b2 = {[0] = 1};
int b3[3] = {.x = 1};
U c2 = {.i = 1, .f = 2};
Q b7 = {.p.q = 1};
