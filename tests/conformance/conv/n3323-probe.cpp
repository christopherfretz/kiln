// [conv.general]/5 (C++14, N3323): contextual implicit conversion to an
// integral or enumeration type picks the unique non-explicit conversion
// function, here in switch, array new and delete. WP-646 feature probe.
//@ expect: run
//@ std: c++14
//@ strict-diags
//@ stdout: 2 4 ok
extern "C" int printf(const char*, ...);
template <class T> struct Wrap {
  T v;
  operator T() const { return v; }
  operator T*() const { return nullptr; }
};
struct Ptr { int* p; operator int*() const { return p; } };
int main() {
  Wrap<int> w{2};
  switch (w) { case 2: printf("2 "); break; default: printf("? "); }
  int* a = new int[Wrap<long>{4}];
  printf("%d ", (int)Wrap<long>{4});
  Ptr p{a};
  delete[] p;
  printf("ok\n");
}
