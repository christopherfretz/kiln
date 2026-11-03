// [basic.life]/1, [expr.const]/5.8: an object of empty class type created
// by `std::construct_at` in allocator storage, alone or as an array
// element, owns its storage: its destruction ends its lifetime, and a
// second destruction is still diagnosed (`std::vector<std::bitset<0>>`).
//@ args: -stdlib=libc++
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:5 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 44:15 note "in call to 'twice()'"
#include <memory>
#include <vector>
struct E {};
constexpr bool one() {
  std::allocator<E> al;
  E *p = al.allocate(1);
  std::construct_at(p);
  p->~E();
  al.deallocate(p, 1);
  return true;
}
static_assert(one());
constexpr bool elems() {
  std::allocator<E> al;
  E *p = al.allocate(3);
  for (int i = 0; i < 3; ++i) std::construct_at(p + i);
  for (int i = 0; i < 3; ++i) p[i].~E();
  al.deallocate(p, 3);
  return true;
}
static_assert(elems());
constexpr bool vec() {
  std::vector<E> v(4);
  v.push_back(E{});
  return v.size() == 5;
}
static_assert(vec());
constexpr bool twice() {
  E e;
  e.~E();
  e.~E();
  return true;
}
static_assert(twice());
