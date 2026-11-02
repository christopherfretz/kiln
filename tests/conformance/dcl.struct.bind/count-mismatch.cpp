//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:8 error "type 'S' decomposes into 2 elements, but only 1 name was provided"
//@ diag: 17:8 error "type 'S' decomposes into 2 elements, but 3 names were provided"
//@ diag: 18:8 error "type 'int[3]' decomposes into 3 elements, but only 2 names were provided"
//@ diag: 19:8 error "type 'E' decomposes into 0 elements, but 1 name was provided"
//@ requires: clang<22
// clang 22 emits error: type 'S' binds to 2 elements, but only 1 name was provided; WP-355 residual
// [dcl.struct.bind]/1 (C++26 /1): the number of identifiers must equal the
// number of elements (array elements, tuple_size, or data members), with
// clang's wording at the `[`.
struct S { int a, b; };
struct E {};
void f(S s, E e) {
  int arr[3] = {};
  auto [a] = s;
  auto [b, c, d] = s;
  auto [g, h] = arr;
  auto [i] = e;
}
