// [temp.spec.partial.member]/1, [temp.mem]/1: an out-of-line partial specialization of a member
// class template of a class template partial specialization (`Outer<T*>::Inner<U&>`)
//@ expect: compile-ok
//@ strict-diags
template <class T>
struct Outer {};
template <class T>
struct Outer<T*> {
  template <class U>
  struct Inner {
    static constexpr int v = 7;
  };
};
template <class T>
template <class U>
struct Outer<T*>::Inner<U&> {
  static constexpr int v = 8;
};
static_assert(Outer<int*>::Inner<char&>::v == 8);
static_assert(Outer<int*>::Inner<char>::v == 7);
int main() {}
