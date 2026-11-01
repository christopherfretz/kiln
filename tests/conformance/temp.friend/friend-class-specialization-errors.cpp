// [temp.friend]/1: only the befriended specialization gets access;
// [temp.friend]/9: a friend declaration cannot declare a partial
// specialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:29 error "partial specialization cannot be declared as a friend"
//@ diag: 18:50 error "'priv' is a private member of 'A<int>'"
//@ diag: 24:25 note "in instantiation of function template specialization 'X<long>::get<int>' requested here"
//@ diag: 15:7 note "implicitly declared private here"
template <class T> struct X;
template <class T> struct Y;
template <class T> class A {
  friend struct X<T>;
  template <class U> friend struct Y<U*>;
  int priv = 1;
};
template <class T> struct X {
  template <class U> int get(A<U>& a) { return a.priv; }
};
int main() {
  A<int> a;
  X<int> ok;
  X<long> no;
  return ok.get(a) + no.get(a);
}
