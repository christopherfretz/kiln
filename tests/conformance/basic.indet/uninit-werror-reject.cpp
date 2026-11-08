// [basic.indet]: a library 'should not compile' test in the style of
// simdjson's iterate_char_star_should_not_compile, which relies on
// -Wuninitialized under -Wall -Werror rejecting a read of an uninitialized
// 'const char *'.
//@ expect: compile-fail
//@ args: -Wall -Werror
//@ strict-diags
//@ diag: 21:42 error "variable 'json' is uninitialized when used here"
//@ diag: 20:19 note "initialize the variable 'json' to silence this warning"
struct padded_string {
  explicit padded_string(const char *s);
};
struct document {};
struct parser {
  document iterate(const padded_string &s);
};

int main() {
  parser p;
  const char *json;
  document doc = p.iterate(padded_string(json));
  (void)doc;
  return 0;
}
