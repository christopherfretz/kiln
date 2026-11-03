// A vector is not an aggregate ([dcl.init.aggr]/1; [dcl.init.list]/3.1), so
// a designated initializer list for one is ill-formed: clang warns about
// each C99 array designator, then rejects the list as a designated
// initialization of a non-aggregate type, at the variable or at a nested
// list's brace. WP-187.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:10 warning "array designators are a C99 extension"
//@ diag: 25:10 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 25:19 note "first non-designated initializer is here"
//@ diag: 25:5 error "initialization of non-aggregate type 'v4i' (vector of 4 'int' values) with a designated initializer list"
//@ diag: 26:13 warning "array designators are a C99 extension"
//@ diag: 26:13 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 26:10 note "first non-designated initializer is here"
//@ diag: 26:5 error "initialization of non-aggregate type 'g4i' (vector of 4 'int' values) with a designated initializer list"
//@ diag: 27:20 warning "array designators are a C99 extension"
//@ diag: 27:20 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 27:17 note "first non-designated initializer is here"
//@ diag: 27:8 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 27:16 note "first non-designated initializer is here"
//@ diag: 27:16 error "initialization of non-aggregate type 'v4i' (vector of 4 'int' values) with a designated initializer list"
typedef int v4i __attribute__((ext_vector_type(4)));
typedef int g4i __attribute__((vector_size(16)));
struct S { int a; v4i v; };
v4i a = {[0] = 1, 2};
g4i b = {1, [2] = 3};
S s = {.a = 1, {1, [1] = 2}};
