// [dcl.init.list]/3.9: list-initialization rejects narrowing
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:7 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 18:7 note "insert an explicit cast to silence this issue"
//@ diag: 19:12 error "constant expression evaluates to -1 which cannot be narrowed to type 'unsigned int'"
//@ diag: 19:12 note "insert an explicit cast to silence this issue"
//@ diag: 20:9 error "non-constant-expression cannot be narrowed from type 'double' to 'float' in initializer list"
//@ diag: 20:9 note "insert an explicit cast to silence this issue"
//@ diag: 21:16 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 21:16 note "insert an explicit cast to silence this issue"
//@ diag: 26:8 error "constant expression evaluates to 2 which cannot be narrowed to type 'bool'"
//@ diag: 26:8 note "insert an explicit cast to silence this issue"
//@ diag: 28:10 error "non-constant-expression cannot be narrowed from type 'long long' to 'int' in initializer list"
//@ diag: 28:10 note "insert an explicit cast to silence this issue"
//@ diag: 31:6 error "constant expression evaluates to 300 which cannot be narrowed to type 'E'"
double d = 2;
int x{d};
unsigned u{-1};
float f{d};
int a[2] = {1, d};
struct S { char c; int i; };
S s{1, 2};
const int k = 7;
char g{k};
bool b{2};
long long ll = 3;
int j = {ll};
enum class E : char { A };
E e1{1};
E e2{300};
