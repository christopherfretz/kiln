// [stmt.pre]/4-5 (C++26, P0963R3): the declarator of a condition shall not
// specify an array, so a structured binding declaration `auto [a, b] = arr`
// as a condition is rejected: one error at the `[`, and no
// `-Wpointer-bool-conversion` on the unnamed variable (kiln warned "address
// of array '' will always evaluate to 'true'" before). `switch` too, and in
// an instantiation. `auto& [a, b] = arr` declares a reference and is
// accepted, as in clang 22 (GCC 16.1 rejects it). Found running GCC's C++
// testsuite.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 21:12 error "an array type is not allowed here"
//@ diag: 22:15 error "an array type is not allowed here"
//@ diag: 23:15 error "an array type is not allowed here"
//@ diag: 24:16 error "an array type is not allowed here"
//@ diag: 28:12 error "an array type is not allowed here"
//@ diag: 30:15 note "in instantiation of function template specialization 'g<int[2]>' requested here"
int arr[2] = {1, 2};
void f() {
  if (auto [a, b] = arr) {}
  while (auto [a, b] = arr) {}
  for (; auto [a, b] = arr;) {}
  switch (auto [a, b] = arr) {}
  if (auto& [a, b] = arr) { (void)a; }
}
template <class T> void g(T& t) {
  if (auto [a, b] = t) {}
}
template void g(int (&)[2]);
