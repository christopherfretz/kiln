// GNU `weakref` / `weakref("target")` on functions and variables (clang
// `GetWeakRefReference`; spec/codegen.md "Aliases and ifuncs"): a use names
// the target, declared `extern_weak` unless the TU references or defines
// it; C++-mangled and `extern "C"` targets.
extern "C" int missing_fn(int);
extern "C" int missing_var;
static int wf(int) __attribute__((weakref("missing_fn")));
static int wv __attribute__((weakref("missing_var")));
static int wm(int) __attribute__((weakref("_Z7mangledi")));
static int wa(int) __attribute__((weakref, alias("_Z7aliasedi")));
int defined_here(int x) { return x; }
static int wd(int) __attribute__((weakref("_Z12defined_herei")));
int use() {
  int r = 0;
  if (wf) r += wf(1);
  if (&wv) r += wv;
  if (wm) r += wm(2);
  if (wa) r += wa(3);
  return r + wd(4);
}
static long wn __attribute__((weakref("_ZN1n1vE")));
long usen() { return &wn ? wn : 0; }
