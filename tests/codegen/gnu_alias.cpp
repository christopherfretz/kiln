// GNU `alias("target")` on functions and variables (clang
// `EmitAliasDefinition`; spec/codegen.md "Aliases and ifuncs"): the target
// is a mangled name, C++-mangled or `extern "C"`; an alias declared before
// its target is defined; `static`, `weak`, hidden and `thread_local`
// aliases; calls and loads name the alias symbol.
extern "C" int impl1(int x) { return x + 1; }
extern "C" int pub1(int) __attribute__((alias("impl1")));
int pub2(int) __attribute__((alias("impl1")));
static int spub(int) __attribute__((alias("impl1")));
int fwd(int) __attribute__((alias("_Z5implbi")));
int implb(int x) { return x * 2; }
static int impls(int x) { return x - 1; }
int pubs(int) __attribute__((alias("_ZL5implsi")));
__attribute__((weak)) int wpub(int) __attribute__((alias("impl1")));
__attribute__((visibility("hidden"))) int hpub(int) __attribute__((alias("impl1")));
namespace ns { int nalias(int) __attribute__((alias("_Z5implbi"))); }
extern "C" int cfwd(int) __attribute__((alias("laterc")));
extern "C" int laterc(int x) { return x + 3; }
int gv = 5;
extern int gva __attribute__((alias("gv")));
static int sgv __attribute__((alias("gv")));
extern const int cgva __attribute__((alias("gv")));
extern "C" double cvar __attribute__((alias("_ZL4dvar")));
static double dvar = 2.5;
thread_local int tv = 1;
extern thread_local int tva __attribute__((alias("tv")));
int use() {
  return pub1(1) + pub2(2) + spub(3) + fwd(4) + pubs(5) + wpub(1) + hpub(2) + ns::nalias(3) +
         cfwd(4) + gva + sgv + cgva + tva + static_cast<int>(cvar);
}
