// Driver codegen flags (spec/driver.md "Codegen flags"):
// -fno-delete-null-pointer-checks makes null a valid address: every
// function gets `null_pointer_is_valid`, reference parameters and returns
// and `this` lose `nonnull` (`this` keeps `dereferenceable_or_null`), and
// nonnull/returns_nonnull attributes and loaded references lose `nonnull`,
// as clang (`CodeGenOptions::NullPointerIsValid`).
//@ args: -fno-delete-null-pointer-checks
struct S { int& r; };
int f(S* s) { return s->r; }
int& g(int** p) { return **p; }
__attribute__((returns_nonnull)) int* h(int* p) { return p; }
void k(int* p) __attribute__((nonnull));
void q(int* p) { k(p); }
__attribute__((nonnull)) void nn(int *p);
__attribute__((returns_nonnull)) int *rn();
struct T { int a; int &ref() { return a; } T(); ~T(); };
int &gr(int &x) { return x; }
void use2() { int *p = new int; nn(p); rn(); T t; t.ref(); delete p; int x; gr(x); }
