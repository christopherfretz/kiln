// [basic.extended.fp] (clang extension): `__fp16` is a storage-only
// type; on x86-64, where the target passes no half values, a function
// parameter or return type of type `__fp16` is an error, written in a
// declarator or produced by substitution (clang
// `GetFullTypeForDeclarator`, `BuildFunctionType`). Pointers and arrays
// (adjusted to pointers) are fine; `_Float16` is unaffected.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 23:8 error "function return value cannot have __fp16 type; did you forget * ?"
//@ diag: 23:17 error "parameters cannot have __fp16 type; did you forget * ?"
//@ diag: 24:14 error "parameters cannot have __fp16 type; did you forget * ?"
//@ diag: 25:10 error "function return value cannot have __fp16 type; did you forget * ?"
//@ diag: 26:20 error "parameters cannot have __fp16 type; did you forget * ?"
//@ diag: 31:34 error "no matching function for call to 'tf'"
//@ diag: 30:22 note "candidate template ignored: substitution failure [with T = __fp16]: function return value cannot have __fp16 type; did you forget * ?"
//@ diag: 33:31 error "parameters cannot have __fp16 type; did you forget * ?"
//@ diag: 34:11 note "in instantiation of template class 'A<__fp16>' requested here"
//@ diag: 33:42 error "function return value cannot have __fp16 type; did you forget * ?"
//@ diag: 35:20 error "parameters cannot have __fp16 type; did you forget * ?"
//@ diag: 36:1 note "in instantiation of template type alias 'Gn' requested here"

__fp16 f(__fp16 x);
void g(__fp16);
__fp16 (*fp)(int);
auto h = [](__fp16 a) { return a; };
__fp16 *ok(__fp16 *p, __fp16 arr[2]);
_Float16 ok2(_Float16);
__fp16 storage[4];
template <class T> T tf(T);
void use() { __fp16 v = 0; (void)tf(v); }

template <class T> struct A { void f(T); T g(); };
A<__fp16> a;
template <class T> using Gn = void(T);
Gn<__fp16> *gnp;
