//@ expect: layout
//@ xfail(kiln): the canned dump puts `i` at offset 2
struct E {};
struct A : E { char c; int i; };
