// [temp.explicit]/8, [temp.func.order]/1: an explicit instantiation
// naming a function template specialization by its signature
// instantiates the most specialized of the templates whose
// specialization matches (by partial ordering, then by constraints,
// [temp.constr.order]), for function templates and member templates
// alike, with or without explicit template arguments.
//@ expect: ir
template<class T> int f(T) { return 1; }
template<class T> int f(T*) { return 2; }
template int f(int*);
template<class T> int g(T, int) { return 1; }
template<class T> int g(T*, int) { return 2; }
template<class T> int g(const T*, int) { return 3; }
template int g(const char*, int);
template int g<char>(char*, int);
template<class T, class U> int h(T, U) { return 1; }
template<class T> int h(T, T) { return 2; }
template int h(long, long);
template<class T> struct A { template<class U> int m(U); template<class U> int m(U*); };
template<class T> template<class U> int A<T>::m(U) { return 1; }
template<class T> template<class U> int A<T>::m(U*) { return 2; }
template int A<int>::m(double*);
template<class T> concept Big = sizeof(T) > 1;
template<class T> concept BigInt = Big<T> && (T(1) / 2 == 0);
template<class T> int k(T) requires Big<T> { return 1; }
template<class T> int k(T) requires BigInt<T> { return 2; }
template int k(long);
