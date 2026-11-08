// [expr.delete]/10 [basic.stc.dynamic.deallocation]/2: under
// -fno-sized-deallocation the sized global deallocation functions are not
// declared implicitly, and a global lookup never prefers a sized one, even
// a declared one: every delete-expression, deleting destructor and
// new-expression cleanup calls the unsized `operator delete`. A class's
// only (sized) `operator delete` still gets the size. WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -fno-sized-deallocation -ftrivial-auto-var-init=uninitialized
using size_t = decltype(sizeof 0);
void operator delete[](void*, size_t) noexcept;
struct A { int x; ~A(); };
struct V { virtual ~V(); int y; };
V::~V() {}
struct T { T(); int z; };
struct C { int x; ~C(); void operator delete(void*, size_t) noexcept; };
struct alignas(64) O { int w; ~O(); };
void f(A* a, int* p, A* arr, C* c, O* o) {
  delete a;
  delete p;
  delete[] arr;
  delete c;
  delete o;
}
A* g() { return new A[3]; }
T* h() { return new T; }
