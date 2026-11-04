// [namespace.udecl]/1, [basic.scope.block], [lex.ext]/5: the literal
// operator a block-scope using-declarator introduces is found only from the
// point of declaration to the end of its block. WP-559.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "no matching literal operator for call to 'operator\"\"_t'"
//@ diag: 21:13 error "no matching literal operator for call to 'operator\"\"_t'"
using size_t = decltype(sizeof 0);
namespace udl {
constexpr unsigned long long operator""_t(const char*, size_t n) { return n; }
}
unsigned long long before() {
  auto a = "x"_t;
  using udl::operator""_t;
  return a + "y"_t;
}
unsigned long long after() {
  {
    using udl::operator""_t;
  }
  return "z"_t;
}
