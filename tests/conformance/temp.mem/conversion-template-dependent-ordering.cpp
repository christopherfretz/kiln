// [temp.dep.expr]/3: a conversion-function-id whose type is dependent is
// type-dependent, so `this->operator T()` and `operator T()` in a member
// template are looked up at instantiation; and conversion function templates
// are ordered by partial ordering ([temp.func.order], clang TPOC_Conversion)
// in an explicit instantiation and in a call `x.operator const T*()`, where
// `operator const T*` is more specialized than `operator T*`. WP-956.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct Foo {
  template <typename T> constexpr operator T() const { return T(7); }
  template <typename T> constexpr T As() const { return this->operator T(); }
  template <typename T> constexpr T As2() const { return operator T(); }
};
template float Foo::As() const;
template double Foo::As2() const;
static_assert(Foo{}.As<int>() == 7 && Foo{}.As2<long>() == 7);
template <class U> struct Baz {
  template <typename T> constexpr operator T() const { return T(sizeof(U)); }
  template <typename T> constexpr T As() const { return this->operator T(); }
};
template float Baz<int>::As() const;
static_assert(Baz<char>{}.As<int>() == 1);
constexpr int one = 1, two = 2;
struct X0 {
  template <typename T> constexpr operator T *() const { return nullptr; }
  template <typename T> constexpr operator const T *() const { return &two; }
};
template X0::operator const int *() const;
static_assert(*X0{}.operator const int *() == 2);
constexpr const int *p = X0{};
static_assert(*p == 2 && &one != p);
int main() {}
