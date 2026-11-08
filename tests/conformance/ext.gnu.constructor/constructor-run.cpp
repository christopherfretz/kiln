// GNU `constructor` / `destructor` (AttrDocs "constructor, destructor";
// WP-807): the functions run from `@llvm.global_ctors` before `main` and
// from `@llvm.global_dtors` after it, lower priorities first for
// constructors and last for destructors. Unprioritized ones run at 65535
// in definition order, before this TU's ordinary dynamic initializers
// (`_GLOBAL__sub_I_<file>`, also 65535, entered last), and their
// destructors after the ordinary objects' ([basic.start.term]/3 runs
// `atexit` registrations first). Static and inline ones are emitted even
// when unused (clang `DeclMustBeEmitted`), as are in-class members.
//@ expect: run
//@ stdout: c101 ip150 c200 c300i S::m S::s cdefa cdefb init main ~init ddef d200 d101
extern "C" int printf(const char*, ...);

struct A {
  const char *n;
  A(const char *s) : n(s) { printf("%s ", n); }
  ~A() { printf("~%s ", n); }
};
A obj("init");
struct P { P() { printf("ip150 "); } };
P p __attribute__((init_priority(150)));

__attribute__((constructor)) void cdefa() { printf("cdefa "); }
__attribute__((constructor(200))) static void c200() { printf("c200 "); }
[[gnu::constructor(101)]] void c101() { printf("c101 "); }
__attribute__((constructor(300))) inline void c300i() { printf("c300i "); }
struct S {
  __attribute__((constructor(400))) static void s() { printf("S::s "); }
  __attribute__((constructor(350))) void m() { printf("S::m "); }
};
__attribute__((constructor)) void cdefb() { printf("cdefb "); }

__attribute__((destructor(101))) void d101() { printf("d101\n"); }
__attribute__((destructor)) static void ddef() { printf("ddef "); }
__attribute__((destructor(200))) void d200() { printf("d200 "); }

int main() { printf("main "); }
