// [temp.func.order]/6.1, /6.2.2: equally specialized templates whose
// template-parameter-lists differ in length, or whose positionally
// corresponding function parameters differ in type, are not ordered by
// their constraints: the call is ambiguous.
//@ verify
template <class T> concept Big = sizeof(T) > 1;
template <class T> requires Big<T> int f(T); // expected-note {{candidate function}}
template <class T, class U = int> int f(T);  // expected-note {{candidate function}}
int a = f(0L); // expected-error {{call to 'f' is ambiguous}}

template <class T> requires Big<T> int g(const T &); // expected-note {{candidate function}}
template <class T> int g(T);                         // expected-note {{candidate function}}
int b = g(0L); // expected-error {{call to 'g' is ambiguous}}
