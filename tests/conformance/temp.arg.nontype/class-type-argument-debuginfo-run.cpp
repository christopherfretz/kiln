// [temp.arg.nontype]/1 at `-g` (WP-910): a class-type template argument
// is the `value:` of its DITemplateValueParameter, an IR constant of the
// class's struct type. When nothing else in the module uses that struct
// type (the parameter object is never odr-used), its definition must
// still be printed: LLVM's printer leaves it
// out (clang 19's own `-emit-llvm -g` text does not parse back), and kiln
// hands text to the backend, which rejected the module.
//@ expect: run
//@ args: -g
//@ stdout: 7 ab 2 3
extern "C" int printf(const char *, ...);
struct V { int v; };
struct Lit { char s[3]; };
struct P { const int *p; };
extern const int g;
const int g = 2;
template <V x> int val() { return x.v; }
template <Lit l> const char *lit() { return l.s; }
template <P p> int deref() { return *p.p; }
struct U { int u; };
struct S { char s[3]; };
struct Q { const int *q; };
template <U u, S s, Q q> int unused() { return 3; }
int main() {
  printf("%d %s %d %d\n", val<V{7}>(), lit<Lit{"ab"}>(), deref<P{&g}>(),
         unused<U{1}, S{"cd"}, Q{&g}>());
}
