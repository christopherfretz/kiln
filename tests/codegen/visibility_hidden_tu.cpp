//@ args: -fvisibility=hidden
// Symbol visibility (spec/codegen.md "Symbol visibility"; clang
// getLVForDecl / setGlobalVisibility): under -fvisibility=hidden every
// definition with external linkage is `hidden` unless an explicit
// `visibility("default")` attribute ([dcl.attr.grammar] GNU extension)
// on it or on its class exports it. A `visibility("default")` extern "C"
// function is the module-init export pattern. Declarations of entities
// defined elsewhere stay default unless known hidden.
struct __attribute__((visibility("default"))) Exported {
  Exported();
  virtual ~Exported();
  virtual int get() const;
  static int count;
  int x = 0;
};
Exported::Exported() {}
Exported::~Exported() {}
int Exported::get() const { return x; }
int Exported::count = 0;

struct Internal {
  virtual int get() const;
};
int Internal::get() const { return 2; }

__attribute__((visibility("default"))) int api(int a) { return a + 1; }
int helper(int a) { return a * 2; }
int global_var = 3;
__attribute__((visibility("default"))) int exported_var = 4;

extern int ext_var;
extern __attribute__((visibility("hidden"))) int ext_hidden;
int ext_fn();
__attribute__((visibility("hidden"))) int ext_hidden_fn();

extern "C" __attribute__((visibility("default"))) void *module_init() {
  static Exported e;
  Internal i;
  return (void *)(long)(api(ext_var) + helper(ext_hidden) + ext_fn() +
                        ext_hidden_fn() + e.get() + i.get() + global_var +
                        exported_var);
}
