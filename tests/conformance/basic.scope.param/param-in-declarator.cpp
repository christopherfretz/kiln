// A parameter is in scope from its declaration to the end of the
// declarator ([basic.scope.param]); kiln does not model that scope
// yet and says so instead of using an outer name.
//@ expect: run
//@ xfail(kiln): parameters used within their function declarator are not supported yet
int n;
auto k(int n) -> decltype(n * 2) { return n * 2; }
int main() { return k(1) - 2; }
