// GNU __null (spec/sema.md §5.8): an integer null pointer constant;
// -Wnull-conversion warns when it converts to a non-pointer scalar type,
// -Wnull-arithmetic when it is an operand of an arithmetic or comparison
// operator. Both are on by default.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 37:9 warning "implicit conversion of NULL constant to 'int'"
//@ diag: 38:10 warning "implicit conversion of NULL constant to 'bool'"
//@ diag: 39:11 warning "implicit conversion of NULL constant to 'char'"
//@ diag: 40:12 warning "implicit conversion of NULL constant to 'double'"
//@ diag: 41:20 warning "implicit conversion of NULL constant to 'unsigned long'"
//@ diag: 47:9 warning "implicit conversion of NULL constant to 'int'"
//@ diag: 51:11 warning "implicit conversion of NULL constant to 'int'"
//@ diag: 54:17 warning "use of NULL in arithmetic operation"
//@ diag: 55:12 warning "use of NULL in arithmetic operation"
//@ diag: 56:12 warning "use of NULL in arithmetic operation"
//@ diag: 57:12 warning "use of NULL in arithmetic operation"
//@ diag: 58:12 warning "use of NULL in arithmetic operation"
//@ diag: 59:12 warning "use of NULL in arithmetic operation"
//@ diag: 60:12 warning "use of NULL in arithmetic operation"
//@ diag: 61:12 warning "use of NULL in arithmetic operation"
//@ diag: 62:5 warning "use of NULL in arithmetic operation"
//@ diag: 63:5 warning "use of NULL in arithmetic operation"
//@ diag: 64:7 warning "implicit conversion of NULL constant to 'int'"
//@ diag: 65:11 warning "implicit conversion of NULL constant to 'bool'"
//@ diag: 67:19 warning "use of NULL in arithmetic operation"
//@ diag: 68:17 warning "comparison between NULL and non-pointer (NULL and 'int')"
//@ diag: 69:12 warning "comparison between NULL and non-pointer ('int' and NULL)"
//@ diag: 70:12 warning "comparison between NULL and non-pointer ('int' and NULL)"
//@ diag: 76:36 warning "implicit conversion of NULL constant to 'int'"
//@ diag: 77:10 note "in instantiation of function template specialization 'tf<int>' requested here"
#define NULL __null
#define MYNULL __null
#define INIT(x) int x = NULL
struct S { int m; };

int i = __null;
bool b = __null;
char c = (__null);
double d = __null;
unsigned long ul = __null;
long l = __null;               // its own type: no warning
int *p = __null;
int S::*mp = __null;
void (*fp)() = NULL;
decltype(nullptr) np = __null;
int j = NULL;                  // reported at NULL
int k = MYNULL;                // another macro: no warning
INIT(m);                       // NULL inside a macro body: no warning
int h(int);
int r = h(__null);

void f(int x, int *q) {
  (void)(__null + 1);
  (void)(1 - __null);
  (void)(x * __null);
  (void)(x << __null);
  (void)(x & __null);
  (void)(x | __null);
  (void)(x ^ __null);
  (void)(q + __null);
  x += __null;
  x <<= __null;
  x = __null;
  (void)(!__null);
  (void)(-__null);
  long z = __null + 0;
  (void)(__null == 0);
  (void)(x != __null);
  (void)(x < __null);
  (void)(q == __null);
  (void)(__null == __null);
  (void)(mp == __null);
}

template <class T> T tf() { return __null; }
int ti = tf<int>();
int *tp = tf<int *>();
