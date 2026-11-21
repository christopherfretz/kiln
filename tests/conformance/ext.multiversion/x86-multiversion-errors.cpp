// [clang ext] Function multiversioning on x86-64 (clang AttrDocs
// "target", "target_clones", "cpu_dispatch", "cpu_specific"; clang
// `Sema::CheckMultiVersionFunction`): the redeclaration rules of the
// versions of one function, the kinds of function that cannot be
// multiversioned, and the version-list checks of `target_clones` and the
// `cpu_*` attributes. spec/sema.md "Function multiversioning" (WP-1745).
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 43:52 error "multiple 'cpu_specific' functions cannot specify the same CPU: 'atom'"
//@ diag: 42:41 note "previous declaration is here"
//@ diag: 45:46 error "'cpu_dispatch' function redeclared with different CPUs"
//@ diag: 44:41 note "previous declaration is here"
//@ diag: 46:16 warning "CPU list contains duplicate entries; attribute ignored"
//@ diag: 47:48 warning "body of cpu_dispatch function will be ignored"
//@ diag: 49:16 error "'target_clones' multiversioning requires a default target"
//@ diag: 51:57 error "'target_clones' attribute does not match previous declaration"
//@ diag: 50:55 note "previous declaration is here"
//@ diag: 54:38 error "multiversioned function declaration has a different return type"
//@ diag: 56:47 error "attribute 'target' multiversioning cannot be combined with attribute 'noinline'"
//@ diag: 58:37 error "multiversioned function declaration has a different constexpr specification"
//@ diag: 61:44 error "multiversioned function redeclarations require identical target attributes"
//@ diag: 60:44 note "previous declaration is here"
//@ diag: 63:47 error "function multiversioning doesn't support architecture 'x86-64-v2'"
//@ diag: 65:5 error "function declaration is missing 'target' attribute in a multiversioned function"
//@ diag: 66:40 error "'main' cannot be a multiversioned function"
//@ diag: 68:40 error "multiversioning attributes cannot be combined"
//@ diag: 67:41 note "previous declaration is here"
//@ diag: 70:5 error "function declaration is missing 'cpu_specific' or 'cpu_dispatch' attribute in a multiversioned function"
//@ diag: 72:37 error "multiversioned function declaration has a different linkage"
//@ diag: 74:40 error "function multiversioning doesn't support feature 'no-avx2'"
//@ diag: 76:40 error "redefinition of 'm16'"
//@ diag: 75:40 note "previous definition is here"
//@ diag: 78:37 error "exception specification in declaration does not match previous declaration"
//@ diag: 77:40 note "previous declaration is here"
//@ diag: 80:59 error "attribute 'target' multiversioned functions do not yet support function templates"
//@ diag: 81:41 error "attribute 'target' multiversioned functions do not yet support deduced return types"
//@ diag: 83:50 error "attribute 'target' multiversioned functions do not yet support virtual functions"
//@ diag: 84:38 error "attribute 'target' multiversioned functions do not yet support constructors"
//@ diag: 85:38 error "attribute 'target' multiversioned functions do not yet support destructors"
// cpu_specific / cpu_dispatch lists.
__attribute__((cpu_specific(atom))) int c1() { return 1; }
__attribute__((cpu_specific(atom, ivybridge))) int c1();
__attribute__((cpu_dispatch(atom))) int c2();
__attribute__((cpu_dispatch(ivybridge))) int c2();
__attribute__((cpu_specific(atom, bonnell))) int c3();
__attribute__((cpu_dispatch(atom))) int c4() { return 1; }
// target_clones.
__attribute__((target_clones("avx2"))) int t1();
__attribute__((target_clones("avx2", "default"))) int t2();
__attribute__((target_clones("sse4.2", "default"))) int t2();
// Versions of one function.
__attribute__((target("default"))) int m1();
__attribute__((target("avx2"))) long m1();
__attribute__((target("default"))) int m2();
__attribute__((target("avx2"), noinline)) int m2();
__attribute__((target("default"))) constexpr int m3() { return 1; }
__attribute__((target("avx2"))) int m3() { return 1; }
__attribute__((target("default"))) int m4();
__attribute__((target("avx2,sse4.2"))) int m4();
__attribute__((target("sse4.2,avx2"))) int m4();
__attribute__((target("default"))) int m6();
__attribute__((target("arch=x86-64-v2"))) int m6();
__attribute__((target("default"))) int m7();
int m7();
__attribute__((target("default"))) int main();
__attribute__((cpu_specific(atom))) int m8();
__attribute__((target("default"))) int m8();
__attribute__((cpu_specific(atom))) int m13();
int m13();
__attribute__((target("default"))) static int m14();
__attribute__((target("avx2"))) int m14();
__attribute__((target("default"))) int m15();
__attribute__((target("no-avx2"))) int m15();
__attribute__((target("default"))) int m16() { return 0; }
__attribute__((target("default"))) int m16() { return 1; }
__attribute__((target("default"))) int m17();
__attribute__((target("avx2"))) int m17() noexcept;
// Functions that cannot be multiversioned.
template <class T> __attribute__((target("default"))) int k1();
__attribute__((target("default"))) auto k2() { return 1; }
struct S {
  __attribute__((target("default"))) virtual int v();
  __attribute__((target("default"))) S();
  __attribute__((target("default"))) ~S();
};
__attribute__((target("default"))) int k3() = delete;
__attribute__((target("default"))) consteval int k4() { return 1; }
__attribute__((target("avx2"))) consteval int k4() { return 2; }
__attribute__((target_clones("avx2", "default"))) int k5() = delete;
