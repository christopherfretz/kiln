// [temp.mem.enum]/1: an enumeration member of a class template may be defined outside the class
// template definition; its enumerators are instantiated with the enclosing specialization
//@ expect: run
//@ strict-diags
template <class T>
struct A {
  enum class E : int;
  enum U : int;
};
template <class T>
enum class A<T>::E : int { a = sizeof(T), b };
template <class T>
enum A<T>::U : int { u0 = 2 * sizeof(T) };
int main() {
  return (int(A<long>::E::b) == 9 ? 0 : 1) + (A<int>::u0 == 8 ? 0 : 2);
}
