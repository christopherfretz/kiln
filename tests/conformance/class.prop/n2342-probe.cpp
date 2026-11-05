// [class.prop] (C++11, N2342): trivial and standard-layout classes, and
// offsetof on standard-layout types. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++11
//@ strict-diags
struct Pod { int a; char b; };
struct Ctor { Ctor() {} int a; };
struct Mixed { public: int a; private: int b; };
struct Base { int a; };
struct DerivedSL : Base { void f(); };
struct DerivedNSL : Base { int b; };
struct Virt { virtual void f(); };
static_assert(__is_trivial(Pod) && __is_standard_layout(Pod), "pod");
static_assert(!__is_trivial(Ctor) && __is_standard_layout(Ctor), "ctor");
static_assert(__is_trivial(Mixed) && !__is_standard_layout(Mixed), "access");
static_assert(__is_standard_layout(DerivedSL), "empty derived");
static_assert(!__is_standard_layout(DerivedNSL), "members in two classes");
static_assert(!__is_trivial(Virt) && !__is_standard_layout(Virt), "virtual");
static_assert(__builtin_offsetof(Pod, b) == sizeof(int), "offsetof");
static_assert(__is_trivially_copyable(Ctor), "trivially copyable");
