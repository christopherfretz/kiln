// [dcl.init.list]/3.10: a reference list-initialized from a list whose single
// element is not reference-related creates a temporary of the referenced type,
// list-initialized with the same (direct or copy) kind as the reference.
// clang names the reference type in a failed constructor call.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:10 error "no matching constructor for initialization of 'const E &'"
//@ diag: 14:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const E' for 1st argument"
//@ diag: 14:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'E' for 1st argument"
//@ diag: 14:12 note "candidate constructor not viable: requires 2 arguments, but 1 was provided"
//@ diag: 19:10 error "chosen constructor is explicit in copy-initialization"
//@ diag: 13:21 note "explicit constructor declared here"
struct D { explicit D(int); };
struct E { E(int, int); };
const D& d3{1};           // direct-list: explicit constructor is fine
const D& d5{D(2)};
const E& e3{1};           // the error names the reference type
const E& e4{1, 2};
const D& d4 = {1};        // copy-list: explicit constructor is an error
