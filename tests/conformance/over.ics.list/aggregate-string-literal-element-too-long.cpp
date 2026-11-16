// [over.ics.list]/8, [dcl.init.string]/2: a string literal of a compatible
// type initializing the character array element of an aggregate forms the
// conversion sequence whatever its length; one longer than the array (its
// terminator included) is then an error of the initialization. Before
// C++14 a class with a default member initializer is no aggregate.
//@ expect: compile-fail
//@ std: c++14
//@ strict-diags
//@ diag: 15:18 error "initializer-string for char array is too long, array size is 4 but initializer has size 5 (including the null terminating character)"

struct A {
  char str[4] = "foo";
};
void take(A);
void g() { take({"abcd"}); }
