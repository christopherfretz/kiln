export module imgme;
export namespace me {
template <class T> struct A {
  enum { a = 1, b, c = sizeof(T) * 2, d = c + b };
  enum E : long { x = a + 10, y };
  enum class S { p = sizeof(T), q };
  int get() const { return d + int(S::q); }
};
template <class T> struct is_int {
  enum { value = 0 };
};
template <> struct is_int<int> {
  enum { value = 1 };
};
template <class T> struct B {
  enum { v = T::value + 1 };
};
} // namespace me
