// [basic.start.term]/1: a static object with a non-trivial destructor
// registers it with __cxa_atexit(dtor, &obj, &__dso_handle) after its
// initialization; a constant-initialized one registers only that. An
// array of class type is destroyed by a __cxx_global_array_dtor helper
// (global_init_array_eh.cpp covers arrays whose construction may throw).
struct A { A(); A(int); ~A(); int x; };
struct D { constexpr D(int v) : x(v) {} ~D(); int x; };
A a1;
A a2(2);
D d(1);
static D sd(2);
D darr[2] = {D(1), D(2)};
int use() { return sd.x; }
