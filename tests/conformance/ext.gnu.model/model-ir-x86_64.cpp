// GNU `model("small" | "large")` on x86-64 (clang 22 `CodeModelAttr`,
// `GetOrCreateLLVMGlobal`): the global of a non-TLS variable gets
// `code_model`, on definitions and declarations, on static data members
// and variable template specializations, after the section; a later
// declaration inherits it, the first attribute of a declaration wins,
// a global an earlier use created keeps no code model, and a static
// local gets none. spec/sema.md §4.5, WP-1745.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
int a __attribute__((model("small"))) = 1;
int b __attribute__((model("large"))) = 2;
[[gnu::model("large")]] int c;
int d __attribute__((__model__("large"), section("cm_data"))) = 4;
int e __attribute__((model("large"))) __attribute__((model("small")));
extern int x __attribute__((model("large")));
struct S { static int m __attribute__((model("large"))); };
int S::m;
namespace N { int z [[gnu::model("large")]]; }
template <class T> T v __attribute__((model("large"))) = T();
extern int g;
int use_g() { return g; }
int g __attribute__((model("large")));
extern int h __attribute__((model("large")));
int h;
int f() { static int s __attribute__((model("large"))) = 3; return s; }
int use() { return x + a + v<int> + h + S::m; }
