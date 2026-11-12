#ifndef GLOBAL_NEW_PAREN_H
#define GLOBAL_NEW_PAREN_H
// The shapes of libstdc++'s <new> and <bits/postypes.h>: replaceable
// global allocation functions redeclared, a placement form defined, and a
// dependent direct-initialization in a class template.
namespace gnp {
using size_t = decltype(sizeof 0);
enum class align_val_t : size_t {};
struct nothrow_t { explicit nothrow_t() = default; };
inline constexpr nothrow_t nothrow{};
} // namespace gnp
void *operator new(gnp::size_t);
void *operator new[](gnp::size_t);
void operator delete(void *) noexcept;
void operator delete[](void *) noexcept;
void *operator new(gnp::size_t, const gnp::nothrow_t &) noexcept;
inline void *operator new(gnp::size_t, void *p) noexcept { return p; }
namespace gnp {
template <class T> struct pos {
  T off;
  pos &operator+=(T d) { off += d; return *this; }
  pos operator+(T d) const { pos p(*this); p += d; return p; }
  template <class U> static pos make(U u) { pos p(u, 0); return p; }
  pos(T o) : off(o) {}
  template <class U> pos(U a, int b) : off(T(a) + b) {}
};
} // namespace gnp
#endif
