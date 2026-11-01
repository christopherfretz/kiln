// [basic.life], [expr.const]/5.18, [expr.new]: in std::allocator storage a
// placement new within namespace std (std::construct_at) begins an object's
// lifetime, and a destructor call (std::destroy_at, explicit or pseudo)
// ends it. Destroying, reading or calling a member function of an object
// outside its lifetime is not constant, nor is a placement new at a null
// pointer or of another type than the storage's. spec/oven.md "Transient
// allocation".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 91:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:54 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 87:3 note "in call to 'destroy_at<int>(&{*new int[1]#0}[0])'"
//@ diag: 91:15 note "in call to 'destroy_twice()'"
//@ diag: 103:15 error "static assertion expression is not an integral constant expression"
//@ diag: 99:11 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 103:15 note "in call to 'read_destroyed()'"
//@ diag: 114:15 error "static assertion expression is not an integral constant expression"
//@ diag: 110:14 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 114:15 note "in call to 'call_unconstructed()'"
//@ diag: 127:15 error "static assertion expression is not an integral constant expression"
//@ diag: 123:14 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 127:15 note "in call to 'explicit_destructor()'"
//@ diag: 134:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:75 note "construction of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 131:3 note "in call to 'construct_at<int, int>(nullptr, 1)'"
//@ diag: 134:15 note "in call to 'construct_null()'"
//@ diag: 144:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:75 note "placement new would change type of storage from 'short' to 'long'"
//@ diag: 140:3 note "in call to 'construct_as<long, short>(&{*new short[4]#0}[0], 1)'"
//@ diag: 144:15 note "in call to 'construct_wrong_type()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T> struct allocator {
  constexpr T *allocate(size_t n) { return static_cast<T *>(operator new(n * sizeof(T))); }
  constexpr void deallocate(T *p, size_t) { operator delete(p); }
};
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
template <class T, class U> constexpr T *construct_as(U *p, T u) { return ::new ((void *)p) T(u); }
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std

struct Counted {
  int *live;
  int v;
  constexpr Counted(int *l) : live(l), v(7) { ++*live; }
  constexpr ~Counted() { --*live; }
  constexpr int get() const { return v; }
};

// Class objects: construct_at runs the constructor, destroy_at the
// destructor, each in the allocator's storage.
constexpr int objects() {
  int live = 0;
  std::allocator<Counted> a;
  Counted *p = a.allocate(3);
  for (int i = 0; i < 3; ++i)
    std::construct_at(p + i, &live);
  int seen = live + p[2].get();
  for (int i = 0; i < 3; ++i)
    std::destroy_at(p + i);
  a.deallocate(p, 3);
  return seen * 10 + live;
}
static_assert(objects() == 100);

// An object may be constructed again after its destruction.
constexpr int reuse() {
  std::allocator<int> a;
  int *p = a.allocate(1);
  std::construct_at(p, 1);
  std::destroy_at(p);
  std::construct_at(p, 2);
  int r = *p;
  std::destroy_at(p);
  a.deallocate(p, 1);
  return r;
}
static_assert(reuse() == 2);

// Destroyed twice.
constexpr int destroy_twice() {
  std::allocator<int> a;
  int *p = a.allocate(1);
  std::construct_at(p, 1);
  std::destroy_at(p);
  std::destroy_at(p);
  a.deallocate(p, 1);
  return 1;
}
static_assert(destroy_twice());

// Read after destruction.
constexpr int read_destroyed() {
  std::allocator<int> a;
  int *p = a.allocate(1);
  std::construct_at(p, 1);
  std::destroy_at(p);
  int r = *p;
  a.deallocate(p, 1);
  return r;
}
static_assert(read_destroyed());

// A member function called on storage with no object.
constexpr int call_unconstructed() {
  int live = 0;
  std::allocator<Counted> a;
  Counted *p = a.allocate(1);
  int r = p->get();
  a.deallocate(p, 1);
  return r;
}
static_assert(call_unconstructed());

// An explicit destructor call ends the lifetime too.
constexpr int explicit_destructor() {
  int live = 0;
  std::allocator<Counted> a;
  Counted *p = a.allocate(1);
  std::construct_at(p, &live);
  p->~Counted();
  int r = p->get();
  a.deallocate(p, 1);
  return r;
}
static_assert(explicit_destructor());

// Placement new at a null pointer.
constexpr int construct_null() {
  std::construct_at((int *)nullptr, 1);
  return 1;
}
static_assert(construct_null());

// Placement new of a type that is not the type of the storage.
constexpr int construct_wrong_type() {
  std::allocator<short> a;
  short *p = a.allocate(4);
  std::construct_as(p, 1L);
  a.deallocate(p, 4);
  return 1;
}
static_assert(construct_wrong_type());
