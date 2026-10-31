// [stmt.return]/2: in a function returning void, the operand may be an
// expression of type void; a type-dependent operand (a pack-expansion
// call in a template) is checked at instantiation. libc++ `<new>`
// `__do_deallocate_handle_size` (`return std::__libcpp_operator_delete(
// __ptr, __size, __args...);`). WP-128.
//@ expect: compile-ok

template <class... A> void g(A...) {}
namespace n {
template <class... A> void h(A...) {}
} // namespace n
template <class... A> void f(void* p, A... a) { return g(p, a...); }
template <class... A> void f2(void* p, A... a) { return n::h(p, a...); }
template <class T> void f3(T t) { return f2(&t, t); }
inline void d(void* p) { return f(p); }
inline void d2(void* p, unsigned long s) { return f2(p, s, 1); }
void d3() { f3(1); }
void v();
void e() { return v(); }

int main() {}
