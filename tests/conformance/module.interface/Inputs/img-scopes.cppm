// The module of image-scopes-run.cpp: linkage specifications (braced,
// braceless, "C" and "C++"), literal exception specifications, inline and unnamed namespaces, reserved names
// in the namespace std, which a new-expression before it has sema declare,
// and builtin functions sema declares at their first use.
export module imgs;

export extern "C" int imgs_c_add(int a, int b) { return a + b; }
export extern "C" {
int imgs_c_mul(int a, int b) { return a * b; }
}
export extern "C++" int imgs_cxx(int a) { return a - 1; }
// Exception specifications written as literals, as a C library header's
// __THROW expands in C++ ([except.spec]/2), and the nothrow attribute.
export extern "C" int imgs_nt(int a) noexcept(true) { return a + 4; }
export int imgs_nf(int a) noexcept(false) { return a + 5; }
export __attribute__((nothrow)) int imgs_na(int a) { return a + 6; }

export namespace lib {
inline namespace v2 {
constexpr int version() { return 2; }
}
namespace v1 {
constexpr int version() { return 1; }
}
inline namespace v2 {
struct Tag {
  int v = 20;
};
}
} // namespace lib

namespace {
int helper(int x) { return x * 3; }
} // namespace
namespace {
int helper2(int x) { return helper(x) + 1; }
} // namespace
export int use_helper(int x) { return helper2(x); }

export int __imgs_global(int __x) { return __x + 100; }

// The new-expression has sema declare its hidden namespace std, which the
// namespace std below extends.
export int heap_val() {
  int *p = new int(5);
  int v = *p;
  delete p;
  return v;
}

// Builtin functions: inline bodies the importer instantiates or evaluates
// call its own declarations of them.
export constexpr unsigned long imgs_len(const char *s) { return __builtin_strlen(s); }
export inline int imgs_clz(unsigned x) { return __builtin_clz(x); }
export int imgs_abs(int x) { return __builtin_abs(x); }

export namespace std {
inline namespace __imgs {
struct __pair_like {
  int __a, __b;
  constexpr int __sum() const { return __a + __b; }
};
constexpr int __twice(int __x) { return __x * 2; }
} // namespace __imgs
} // namespace std
