//@ args: -Oz
// -Oz (clang `CodeGenOpts.OptimizeSize` 2, `getTrivialDefaultFunctionAttributes`):
// every function definition and declaration, and every call site through
// `EmitCall` (direct, indirect, constructor, destructor, `operator new`/
// `delete`), gets `minsize optsize`, unless it is `optnone` (an `optnone`
// definition drops it; a declaration or call of an `optnone` function
// never gets it). Runtime functions (`__cxa_atexit`,
// `__cxa_allocate_exception`, `__cxa_throw`), intrinsics and the implicit
// calls of `__cxx_global_var_init` do not. Initialization functions are
// `noinline` only at -O0 (spec/codegen.md "Size optimization attributes").
int ext(int);
int f(int x) { return ext(x) + 1; }
__attribute__((optnone)) int on(int x) { return ext(x); }
__attribute__((optnone)) int on_decl(int x);
__attribute__((minsize)) int ms(int x) { return ext(x); }
__attribute__((cold)) int cd(int x) { return ext(x); }
int calls(int x) { return on(x) + on_decl(x) + ms(x) + cd(x); }
int indirect(int (*p)(int), int x) { return p(x); }
struct E {};
E *mk() { return new E; }
void del(E *p) { delete p; }
struct G {
  G();
  ~G();
};
G g;
void thr() { throw E(); }
