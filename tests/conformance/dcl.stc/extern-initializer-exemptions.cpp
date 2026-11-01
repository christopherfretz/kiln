// clang's -Wextern-initializer: an 'extern' variable with an initializer,
// except a const object, an extern "C" variable or an instantiation.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 14:12 warning "explicit specialization cannot have a storage class"
//@ diag: 19:30 warning "'extern' variable has an initializer"
//@ diag: 20:30 warning "'extern' variable has an initializer"
//@ diag: 21:12 warning "explicit specialization cannot have a storage class"
//@ diag: 21:23 warning "'extern' variable has an initializer"
//@ diag: 23:12 warning "'extern' variable has an initializer"
// A const object, an extern "C" variable and an instantiation are exempt.
template<class T> extern const int v = 1;
template<class T> extern const int v<T*> = 2;
template<> extern const int v<int> = 3;
static_assert(v<char> == 1 && v<int*> == 2 && v<int> == 3);
template<class T> constexpr int w = 1;
template<class T> constexpr int w<T*> = 2;
static_assert(w<int*> == 2);
template<class T> extern int x = 1;
template<class T> extern int x<T*> = 2;
template<> extern int x<int> = 3;
int use = x<char> + x<int*> + x<int>;
extern int y = 1;
extern const int z = 1;
typedef const int CI;
extern CI za[2] = {1, 2};
extern "C" { extern int yd = 1; }
template int x<long>;
