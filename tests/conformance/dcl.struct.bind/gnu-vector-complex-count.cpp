//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 15:8 error "2 elements, but 3 names were provided"
//@ diag: 16:8 error "2 elements, but only 1 name was provided"
//@ diag: 17:9 error "4 elements, but 5 names were provided"
// GNU extension: a vector decomposes into its element count and a complex
// type into two elements; any other count is the usual error (clang 19
// says "decomposes into", clang 22 "binds to"; kiln follows its
// -fclang-compat level). Found running GCC's C++ testsuite.
typedef int V2 __attribute__((vector_size(2 * sizeof(int))));
typedef int V4 __attribute__((vector_size(4 * sizeof(int))));

void f(V2 v, _Complex float z, V4 w) {
  auto [a, b, c] = v;
  auto [m] = z;
  auto &[p, q, r, s, t] = w;
}
