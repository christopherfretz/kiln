// A parameter is in scope from its declaration to the end of the
// declarator ([basic.scope.param]): the trailing return type names the
// parameter, not the outer variable.
//@ expect: run
double n;
auto k(int n) -> decltype(n * 2) { return n * 2; }
static_assert(__is_same(decltype(k(1)), int));
int main() { return k(1) - 2; }
