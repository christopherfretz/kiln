// [temp.param]/14, [basic.def.odr]/5, [expr.prim.lambda.capture]/7: a
// default template argument substituted during deduction is its own
// full-expression; it must not resolve the enclosing call's pending
// lvalue-to-rvalue conversions, so `Size` (a constexpr local used only as
// a constant) is not odr-used and needs no capture (libc++
// list/exception_safety's allocator `allocate(Size)` from a lambda).
//@ expect: compile-ok
//@ strict-diags
template <class T> concept C = sizeof(T) > 0;
template <class T> requires C<T> void h1(T, unsigned long) {}
template <class T> auto h2(T t, unsigned long) -> decltype(t + 1) { return t; }
template <class T> void h3(T, unsigned long) noexcept(sizeof(T) > 1) {}
template <class T, int N = sizeof(T)> void h4(T, unsigned long) {}
struct X { template <class T, int N = sizeof(T)> X(T, unsigned long) {} };
void g() {
  constexpr int Size = 5;
  [] { h1(1, Size); h2(1, Size); h3(1, Size); h4(1, Size); X x(1, Size); }();
}
