// [temp.spec.partial.member]/1, [temp.mem]/1: an out-of-line partial specialization of a member
// class template declared after `Outer<int>` is instantiated is not part of that specialization
// (as clang: the member template was redeclared without it), but is of later ones
//@ expect: compile-ok
//@ strict-diags
template <class T>
struct Outer {
  template <class U>
  struct Inner {
    static constexpr int v = 0;
  };
};
Outer<int> early;
template <class T>
template <class U>
struct Outer<T>::Inner<U*> {
  static constexpr int v = 1;
};
static_assert(Outer<int>::Inner<char*>::v == 0);
static_assert(Outer<long>::Inner<char*>::v == 1);
int main() {}
