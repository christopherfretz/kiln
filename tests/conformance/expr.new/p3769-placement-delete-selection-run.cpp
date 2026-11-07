// [expr.new]/28 as clarified by P3769R1 (a DR): when the initialization
// of a placement new-expression throws, the deallocation function is
// selected from the lookup result: each template is replaced by the
// specialization deduced against `void(void*, P2, ..., Pn)`, candidates
// with another parameter-type-list are removed (a trailing parameter
// with a default argument never matches), then those with unsatisfied
// constraints. Exactly one remaining is called; with several none is
// (clang warns, -Wambiguous-delete). These are the paper's examples.
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ diag: 67:5 warning "multiple suitable 'operator delete' functions for 'M'; no 'operator delete' function will be invoked if initialization throws an exception"
//@ diag: 51:34 note "member 'operator delete' declared here"
//@ diag: 52:15 note "member 'operator delete' declared here"
//@ diag: 58:5 warning "multiple suitable 'operator delete' functions for 'T'; no 'operator delete' function will be invoked if initialization throws an exception"
//@ diag: 64:18 note "in instantiation of function template specialization 'probe<B<0>>' requested here"
//@ diag: 35:23 note "member 'operator delete' declared here"
//@ diag: 36:6 note "member 'operator delete' declared here"
//@ stdout: template A
//@ stdout: none B
//@ stdout: non-template C
//@ stdout: none D
//@ stdout: none M
#include <cstddef>
#include <cstdio>
alignas(16) unsigned char buf[64];
int calls = 0;
struct T { T() { throw 1; } };
// Example 1: a template is a candidate.
struct A {};
void* operator new(std::size_t, A&) { return buf; }
template <int = 0> void operator delete(void*, A&) noexcept { ++calls; std::puts("template A"); }
// Example 2: a template and a non-template both match: none is selected.
template <int I> struct B {};
template <int I> void operator delete(void*, B<I>&) noexcept { ++calls; std::puts("template B"); }
void operator delete(void*, B<0>&) noexcept { ++calls; std::puts("non-template B"); }
void* operator new(std::size_t, B<0>&) { return buf; }
// Example 3: the constraint removes the template.
template <int I> struct C {};
template <int I> requires(I > 0) void operator delete(void*, C<I>&) noexcept { ++calls; std::puts("template C"); }
void operator delete(void*, C<0>&) noexcept { ++calls; std::puts("non-template C"); }
void* operator new(std::size_t, C<0>&) { return buf; }
// A trailing parameter with a default argument does not match.
struct D {};
void* operator new(std::size_t, D&) { return buf; }
void operator delete(void*, D&, int = 0) noexcept { ++calls; std::puts("default D"); }
// Class scope: two suitable members.
struct M {
  M() { throw 2; }
  static void* operator new(std::size_t, int) { return buf; }
  template <class U> static void operator delete(void*, U) noexcept { ++calls; std::puts("template M"); }
  static void operator delete(void*, int) noexcept { ++calls; std::puts("non-template M"); }
  static void operator delete(void*) noexcept {}
};
template <class P> void probe(P& p, const char* name) {
  int before = calls;
  try {
    new (p) T;
  } catch (int) {}
  if (calls == before) std::printf("none %s\n", name);
}
int main() {
  A a; B<0> b; C<0> c; D d;
  probe(a, "A"); probe(b, "B"); probe(c, "C"); probe(d, "D");
  int before = calls;
  try {
    new (1) M;
  } catch (int) {}
  if (calls == before) std::puts("none M");
}
