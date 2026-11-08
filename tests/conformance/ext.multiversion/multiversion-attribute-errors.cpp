// [clang ext] Function multiversioning on AArch64 (clang AttrDocs
// "target_version", "target_clones", "cpu_dispatch", "cpu_specific")
// without the `fmv` target feature, which clang's driver turns off unless
// the runtime library is compiler-rt: the argument checks, the mutual
// exclusion on one declaration, the unchecked `target_clones`, the
// rejected `cpu_*` names, a non-string argument, and a non-default
// `target_version` version that is never declared. spec/landed/codegen/WP-810.md (WP-810).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 50:31 warning "unsupported '' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 51:31 warning "unsupported 'SVE' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 52:16 error "'target_version' attribute takes one argument"
//@ diag: 53:16 error "'target_version' attribute takes one argument"
//@ diag: 54:31 error "expected string literal as argument of 'target_version' attribute"
//@ diag: 55:16 error "'target_version' attribute only applies to functions"
//@ diag: 59:59 error "'target_clones' and 'target_version' attributes are not compatible"
//@ diag: 59:16 note "conflicting attribute is here"
//@ diag: 60:55 error "'target' and 'target_version' attributes are not compatible"
//@ diag: 60:16 note "conflicting attribute is here"
//@ diag: 61:47 error "'target_version' and 'target' attributes are not compatible"
//@ diag: 61:16 note "conflicting attribute is here"
//@ diag: 62:47 error "'target_clones' and 'target' attributes are not compatible"
//@ diag: 62:16 note "conflicting attribute is here"
//@ diag: 71:30 error "expected string literal as argument of 'target_clones' attribute"
//@ diag: 72:16 error "'target_clones' attribute only applies to functions"
//@ diag: 77:29 error "invalid option 'generic' for cpu_dispatch"
//@ diag: 78:16 error "'cpu_dispatch' attribute takes at least 1 argument"
//@ diag: 79:16 error "'cpu_specific' attribute takes at least 1 argument"
//@ diag: 80:16 error "'cpu_dispatch' attribute requires an identifier"
//@ diag: 81:16 error "'cpu_specific' attribute requires an identifier"
//@ diag: 82:16 warning "'cpu_dispatch' attribute only applies to functions"
//@ diag: 83:55 error "'cpu_dispatch' and 'target_version' attributes are not compatible"
//@ diag: 83:16 note "conflicting attribute is here"
//@ diag: 84:29 error "invalid option 'atom' for cpu_specific"
//@ diag: 85:29 error "invalid option 'generic' for cpu_dispatch"
//@ diag: 86:23 error "invalid option 'generic' for cpu_dispatch"
//@ diag: 91:22 error "use of undeclared identifier 'h1'"
//@ diag: 93:23 error "use of undeclared identifier 'h2'"
//@ diag: 95:31 error "expected string literal as argument of 'target_version' attribute"
//@ diag: 96:23 error "expected string literal as argument of 'target' attribute"
//@ diag: 97:27 error "expected string literal as argument of '__target__' attribute"
//@ diag: 98:35 error "expected string literal as argument of 'target_clones' attribute"
//@ diag: 99:30 error "expected string literal as argument of 'target_clones' attribute"
//@ diag: 100:22 error "expected string literal as argument of 'target_clones' attribute"
//@ diag: 101:31 error "expected string literal as argument of 'target_version' attribute"
//@ diag: 102:28 error "use of undeclared identifier 'sve'"
// target_version: one string literal; each '+' part `default` or an FMV
// extension name, else a warning and an ordinary function.
__attribute__((target_version(""))) int b1() { return 1; }
__attribute__((target_version("SVE"))) int b3() { return 1; }
__attribute__((target_version("sve", "sve2"))) int b6() { return 1; }
__attribute__((target_version)) int b7() { return 1; }
__attribute__((target_version(1))) int b8() { return 1; }
__attribute__((target_version("sve"))) int v1;
int use_b() { return b1() + b3() + b6() + b7() + b8(); }

// Mutual exclusion on one declaration, in source order.
__attribute__((target_version("default"))) __attribute__((target_clones("sve","default"))) int b9() { return 1; }
__attribute__((target_version("sve"))) __attribute__((target("sve"))) int b10() { return 1; }
__attribute__((target("sve"))) __attribute__((target_version("sve"))) int b11() { return 1; }
__attribute__((target("sve"))) __attribute__((target_clones("sve"))) int g6();
__attribute__((target_clones("sve"))) __attribute__((target("sve"))) int g7();
__attribute__((target_clones("sve"), target_version("sve"))) int g10();
__attribute__((target_version("sve"))) __attribute__((target_version("sve2"))) int g11();
__attribute__((target("sve"))) __attribute__((target("sve2"))) int g12();

// target_clones: string literals, otherwise unchecked without FMV.
__attribute__((target_clones("sve","sve","default"))) int c1() { return 1; }
__attribute__((target_clones)) int c6() { return 1; }
__attribute__((target_clones(sve))) int c7() { return 1; }
__attribute__((target_clones("sve"))) int v2;
__attribute__((target_clones("sve","default"))) int c8();
__attribute__((target_clones("sve2","default"))) int c8();

// cpu_dispatch / cpu_specific: identifiers; no CPU name is valid here.
__attribute__((cpu_dispatch(generic, atom))) int c11();
__attribute__((cpu_dispatch)) int c12();
__attribute__((cpu_specific)) int c13();
__attribute__((cpu_dispatch(1))) int g1();
__attribute__((cpu_specific("x"))) int g2();
__attribute__((cpu_dispatch(generic))) int v3;
__attribute__((target_version("sve"))) __attribute__((cpu_dispatch(generic))) int g4();
__attribute__((cpu_specific(atom))) __attribute__((target_clones("sve"))) int g5();
__attribute__((cpu_dispatch(generic))) __attribute__((target("sve"))) int g8();
[[clang::cpu_dispatch(generic)]] int g9();

// A non-default version is not declared and its body is not checked.
__attribute__((target_version("sve"))) int h1(int) { static_assert(false); return 2; }
__attribute__((target_version("sve2+default"))) int h1(int) { return 3; }
int use_h() { return h1(0); }
template <class T> __attribute__((target_version("sve"))) T h2(T x) { static_assert(sizeof(T) == 0); return x; }
int use_h2() { return h2(0); }
// A non-string argument is not parsed as an expression.
__attribute__((target_version(sve))) int s1();
__attribute__((target(sve))) int s2();
__attribute__((__target__(sve))) int s3();
__attribute__((target_clones("a", sve, "b"))) int s4();
__attribute__((target_clones(sve+1))) int s5();
[[gnu::target_clones(1, sve)]] int s6();
__attribute__((target_version(sve, "x"))) int s7();
__attribute__((target("x", sve))) int s8();
int s9() { return s1() + s2() + s3() + s4() + s5() + s6() + s7() + s8(); }
