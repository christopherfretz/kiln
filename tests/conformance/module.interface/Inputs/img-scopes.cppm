// The module of image-scopes-run.cpp: linkage specifications (braced,
// braceless, "C" and "C++"), inline and unnamed namespaces, reserved names
// in the namespace std, which a new-expression before it has sema declare.
export module imgs;

export extern "C" int imgs_c_add(int a, int b) { return a + b; }
export extern "C" {
int imgs_c_mul(int a, int b) { return a * b; }
}
export extern "C++" int imgs_cxx(int a) { return a - 1; }

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

export namespace std {
inline namespace __imgs {
struct __pair_like {
  int __a, __b;
  constexpr int __sum() const { return __a + __b; }
};
constexpr int __twice(int __x) { return __x * 2; }
} // namespace __imgs
} // namespace std
