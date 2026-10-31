// [temp.friend]/1.3: a friend template-id must name a function template
// found by lookup whose specialization the declared type deduces; in a
// class template the template must be found at the definition and the
// deduction succeed for each specialization. A befriended
// specialization grants nothing to the template's other
// specializations ([class.access]).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:43 error "no candidate function template was found for dependent friend function template specialization"
//@ diag: 23:43 error "no candidate function template was found for dependent friend function template specialization"
//@ diag: 22:6 note "candidate ignored: not a function template"
//@ diag: 24:24 error "no function template matches function template specialization 'q'"
//@ diag: 26:24 error "no function template matches function template specialization 'r'"
//@ diag: 25:25 note "candidate template ignored: could not match 'T *' against 'D'"
//@ diag: 27:43 error "no function template matches function template specialization 'r'"
//@ diag: 28:8 note "in instantiation of template class 'E<int>' requested here"
//@ diag: 25:25 note "candidate template ignored: could not match 'T *' against 'E<int>'"
//@ diag: 35:83 error "'v' is a private member of 'Box<int>'"
//@ diag: 37:15 note "in instantiation of function template specialization 'cross<long>' requested here"
//@ diag: 32:5 note "implicitly declared private here"
template <class T> struct A { friend void nope<>(A); };
void h(int);
template <class T> struct B { friend void h<>(B); };
struct C { friend void q<>(C); };
template <class T> void r(T*);
struct D { friend void r<>(D); };
template <class T> struct E { friend void r<>(E); };
E<int> e;
template <class T> class Box;
template <class T> int cross(const Box<T>&, const Box<int>&);
template <class T> class Box {
  T v = T();
  friend int cross<>(const Box&, const Box<int>&);
};
template <class T> int cross(const Box<T>& a, const Box<int>& o) { return a.v + o.v; }
int use_ok = cross(Box<int>(), Box<int>());
int use_bad = cross(Box<long>(), Box<int>());
