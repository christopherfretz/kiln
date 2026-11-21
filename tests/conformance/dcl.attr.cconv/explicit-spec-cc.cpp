// [temp.expl.spec] [clang ext] A calling-convention attribute on an explicit
// specialization or instantiation of a function template, or on an
// explicit specialization of a member of a class template, is part of the
// declared function type (no "only applies to functions" warning): the
// template's convention is adopted for the match (clang
// `adjustCCAndNoReturn`), and the specialization has it, written again or
// not. An explicit instantiation's written convention is not checked;
// GNU `noreturn` is adopted both ways. Reduced from real code. WP-1843.
//@ expect: compile-ok
//@ strict-diags
#define PM __attribute__((preserve_most))
#define PA __attribute__((preserve_all))
template <int N> PM void g() {}
template <> PM void g<2>() {}
template <> void g<3>() {}
template <int N> [[clang::preserve_most]] void q() {}
template <> [[clang::preserve_most]] void q<1>() {}
template <> void q<2>() {}
template void q<3>();
template <int N> void f() {}
template void f<1>() PM;
template void g<4>() PA;
template <class T> __attribute__((swiftcall)) void s(T) {}
template <> void s(int) {}
template <> void s<char>(char) __attribute__((swiftcall)) {}
template <> __attribute__((swiftcall)) void s(long);
template <class T> struct A { PM void g(); template <class U> PM void h(U); };
template <> void A<int>::g() {}
template <> PM void A<long>::g() {}
template <> template <> void A<int>::h(int) {}
struct B { template <class U> PM static void h(U); };
template <> void B::h(int) {}
template <> PM void B::h(long) {}
template <int N> __attribute__((noreturn)) void n() { while (true) {} }
template <> void n<1>() { while (true) {} }
template <int N> void n2() {}
template <> __attribute__((noreturn)) void n2<1>() { while (true) {} }
using PMF = PM void (*)();
PMF p1 = &g<2>, p2 = &g<3>, p3 = &q<2>;
static_assert(__is_same(decltype(&g<3>), PMF));
static_assert(__is_same(decltype(&A<int>::g), PM void (A<int>::*)()));
void use() {
  g<2>();
  g<3>();
  q<1>();
  s(1);
  s('c');
  A<int>().g();
  B::h(1);
}
