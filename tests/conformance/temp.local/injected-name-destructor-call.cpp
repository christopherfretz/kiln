// [temp.local]/1, [expr.prim.id.dtor]/2: inside a class template (or a
// partial specialization, or an out-of-line member), `~S` names the
// injected-class-name, so `this->~S()` destroys the current
// specialization (libc++ <future> `__assoc_state::__on_zero_shared`).
//@ expect: run
//@ strict-diags
inline void* operator new(__SIZE_TYPE__, void* p) noexcept { return p; }
int dtors = 0;
template <class T, class A> struct S { void f(); ~S() { ++dtors; } };
template <class T, class A> void S<T, A>::f() { this->~S(); }
template <class T> struct S<T&, int> { void f(); ~S() { ++dtors; } };
template <class T> void S<T&, int>::f() { this->~S(); }
template <class T, class A> struct D : S<T, A> {
  void g() { this->~D(); }
  void h();
};
template <class T, class A> void D<T, A>::h() { this->D::~D(); }
int main() {
  alignas(D<int, long>) unsigned char buf[sizeof(D<int, long>)];
  (new (buf) S<int, long>)->f();
  (new (buf) S<int&, int>)->f();
  (new (buf) D<int, long>)->g();
  (new (buf) D<int, long>)->h();
  return dtors == 4 ? 0 : 1;
}
