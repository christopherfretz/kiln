// [stmt.expand]/1, /5.1 (P1306R5): an expansion-init-list is an
// expression-list, so an element may be a braced-init-list but not
// designated (clang parses it as an expression-list: "expected
// expression"). A braced element initializes the for-range-declaration
// as `decl = {...};` would, with that declaration's diagnostics: excess
// elements, narrowing, an explicit constructor in copy-list-
// initialization, and `auto` deducing std::initializer_list (here without
// <initializer_list>), which a structured binding cannot use.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 22:38 error "expected expression"
//@ diag: 23:42 error "cannot deduce type of initializer list because std::initializer_list was not found"
//@ diag: 24:41 error "excess elements in struct initializer"
//@ diag: 25:35 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 25:35 note "insert an explicit cast to silence this issue"
//@ diag: 25:35 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 26:31 error "excess elements in scalar initializer"
//@ diag: 28:29 error "chosen constructor is explicit in copy-initialization"
//@ diag: 27:21 note "explicit constructor declared here"
struct P { int x, y; };
void f1() { template for (auto x : {.a = 1}) {} }
void f2() { template for (auto [a, b] : {{1, 2}}) {} }
void f3() { template for (P p : {{1, 2, 3}}) {} }
void f4() { template for (P p : {{1.5, 2}}) {} }
void f6() { template for (int i : {{1, 2}}) { (void)i; } }
struct E { explicit E(int); };
void f7() { template for (E e : {{1}}) { (void)e; } }
