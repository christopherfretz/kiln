//@ expect: ast
int decl_only(int, char);
int decl_only(int a, char b) { return a + b; }
void forward();
void forward() {}
static int helper(void) { return 1; }
int main() { forward(); return decl_only(1, 'a') + helper(); }
