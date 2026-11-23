// [temp.friend]/1, [basic.stc.dynamic.general]/2 (WP-2540): the global
// allocation and deallocation functions defined after (and before) a class
// template instantiation whose pattern befriends them are one function
// each: one definition, and the new- and delete-expressions call it.
//@ expect: ir
using size_t = decltype(sizeof 0);
extern "C" void *malloc(size_t);
extern "C" void free(void *);

template <class T> struct A {
  friend void *operator new(size_t);
  friend void operator delete(void *) noexcept;
};
A<int> a;
void *operator new(size_t n) { return malloc(n); }
void operator delete(void *p) noexcept { free(p); }

template <class T> struct B {
  friend void *operator new[](size_t);
  friend void operator delete[](void *) noexcept;
};
void *operator new[](size_t n) { return malloc(n); }
void operator delete[](void *p) noexcept { free(p); }
B<int> b;

int *one() { return new int(1); }
void drop(int *p) { delete p; }
int *many() { return new int[3]; }
void drop_many(int *p) { delete[] p; }
