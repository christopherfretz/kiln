// [cpp.pragma] '#pragma GCC visibility' and namespaces (spec/sema.md
// "Visibility attributes"; clang PopPragmaVisibility): a namespace with a
// visibility attribute suspends the pragma stack in its body, so a pop
// there matches no push (noting where the namespace starts), and a push
// left open at its '}' is an error (noting where it ends) that pops it.
// A namespace without the attribute is transparent to the stack.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:13 error "#pragma visibility pop with no matching #pragma visibility push"
//@ diag: 15:50 note "surrounding namespace with visibility attribute starts here"
//@ diag: 22:13 error "#pragma visibility push with no matching #pragma visibility pop"
//@ diag: 24:1 note "surrounding namespace with visibility attribute ends here"
//@ diag: 25:13 error "#pragma visibility pop with no matching #pragma visibility push"
#pragma GCC visibility push(hidden)
namespace __attribute__((visibility("default"))) N {
#pragma GCC visibility pop
void f();
}
#pragma GCC visibility pop
namespace __attribute__((visibility("hidden"))) M {
#pragma GCC visibility push(default)
#pragma GCC visibility push(protected)
void g();
}
#pragma GCC visibility pop
namespace P {
#pragma GCC visibility push(default)
}
#pragma GCC visibility pop
