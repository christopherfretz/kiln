// C23 6.2.2 (linkage), 6.2.4p3 (static storage): in C no name is mangled.
// External and internal functions and objects, a block-scope static
// (clang names it `<function>.<name>`) and main keep their identifiers.
//@ expect: ir
//@ min-oracle: 22
int ext_var = 1;
static int int_var = 2;
const char *msg = "hi";
static int helper(int x) {
  static int count;
  return x + ++count + int_var;
}
int api(int x) { return helper(x) + ext_var; }
int main(void) { return api(0) - 4 + (msg[0] == 'h'); }
