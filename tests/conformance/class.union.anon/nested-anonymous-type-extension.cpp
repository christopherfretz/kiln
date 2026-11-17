// [class.union.anon]/1: an unnamed class defined in an anonymous union or
// struct (an anonymous struct or union, or one with a declarator) is the
// extension clang reports under -Wnested-anon-types (also a pedantic
// warning in clang; kiln reports it only when named, spec/sema-class.md);
// a named one is an error (member-specification-only-data-members.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++26
//@ args: -Wnested-anon-types
//@ diag: 17:5 warning "anonymous types declared in an anonymous union are an extension"
//@ diag: 18:5 warning "anonymous types declared in an anonymous union are an extension"
//@ diag: 21:23 warning "anonymous types declared in an anonymous union are an extension"
struct S {
  union {
    int a;
    struct { int x; } s;
    union { int y; char z; };
  };
};
static union { int b; struct { int w; } t; };
