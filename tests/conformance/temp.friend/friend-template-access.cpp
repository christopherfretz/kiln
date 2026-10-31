// [temp.friend]/1, [class.access]: only the befriended template's
// specializations get access; a function template with another
// signature, or a class template of another name, does not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:55 error "'k' is a private member of 'Key'"
//@ diag: 24:7 note "implicitly declared private here"
//@ diag: 20:55 error "'v' is a private member of 'Box<int>'"
//@ diag: 34:20 note "in instantiation of function template specialization 'poke<int>' requested here"
//@ diag: 15:5 note "implicitly declared private here"
//@ diag: 22:70 error "'v' is a private member of 'Box<int>'"
//@ diag: 34:67 note "in instantiation of member function 'Writer<int>::get' requested here"
//@ diag: 15:5 note "implicitly declared private here"
template <class T> class Box {
  T v = T();
  template <class U> friend U peek(const Box<U>&);
  template <class U> friend struct Reader;
};
template <class T> T peek(const Box<T>& b) { return b.v; }
template <class T> T poke(const Box<T>& b) { return b.v; }
template <class T> struct Reader { T get(const Box<T>& b) { return b.v; } };
template <class T> struct Writer { T get(const Box<T>& b) { return b.v; } };
class Key {
  int k = 1;
  template <class T> friend void use(T, Key);
};
template <class T> void use(T, Key x) { (void)x.k; }
template <class K> void use(K* p, Key x, K) { (void)x.k; }
int main() {
  Box<int> b;
  int n = 0;
  use(1, Key());
  use(&n, Key(), 0);
  return peek(b) + poke(b) + Reader<int>().get(b) + Writer<int>().get(b);
}
