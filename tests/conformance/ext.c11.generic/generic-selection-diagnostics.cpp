// C11 6.5.1.1 (_Generic in C++, a clang extension; clang 19/22
// ParseGenericSelectionExpression, CreateGenericSelectionExpr): no match
// and no default, two associations of the same type (a typedef too, also
// on instantiation), a duplicate default, a function or reference
// association type, an array association that can never be selected
// (-Wunreachable-code-generic-assoc), and the parse errors. A type
// operand matches as written (const int), relaxing the object-type
// checks; an incomplete association type is in the -19 and -c2y tests.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 33:18 error "controlling expression type 'int' not compatible with any generic association type"
//@ diag: 34:29 error "type 'int' in generic association compatible with previously specified type 'int'"
//@ diag: 34:21 note "compatible type 'int' specified here"
//@ diag: 35:41 error "duplicate default generic association"
//@ diag: 35:29 note "previous default generic association is here"
//@ diag: 36:21 error "type 'void ()' in generic association not an object type"
//@ diag: 37:21 error "type 'int &' in generic association not an object type"
//@ diag: 40:29 error "type 'I' (aka 'int') in generic association compatible with previously specified type 'int'"
//@ diag: 40:21 note "compatible type 'int' specified here"
//@ diag: 42:21 warning "due to lvalue conversion of the controlling expression, association of type 'int[3]' will never be selected because it is of array type"
//@ diag: 43:19 error "expected ','"
//@ diag: 44:24 error "expected ':'"
//@ diag: 45:21 error "expected a type"
//@ diag: 46:26 error "use of undeclared identifier 'nope'"
//@ diag: 47:46 error "controlling expression type 'char' not compatible with any generic association type"
//@ diag: 48:9 note "in instantiation of function template specialization 't1<char>' requested here"
//@ diag: 49:58 error "type 'int' in generic association compatible with previously specified type 'int'"
//@ diag: 50:9 note "in instantiation of function template specialization 't2<int>' requested here"
//@ diag: 49:52 note "compatible type 'int' specified here"
struct Inc;
int x;
int a = _Generic(x, long: 1);
int b = _Generic(x, int: 1, int: 2);
int c = _Generic(x, int: 1, default: 2, default: 3);
int e = _Generic(x, void(): 1, default: 2);
int f = _Generic(x, int&: 1, default: 2);
int g = _Generic(int, int: 1, const int: 2, long: 3);
typedef int I;
int h = _Generic(x, int: 1, I: 2);
int i = _Generic(Inc, Inc: 1, default: 2);
int j = _Generic(x, int[3]: 1, default: 2);
int k = _Generic(x int: 1);
int l = _Generic(x, int 1);
int m = _Generic(x, );
int n = _Generic(x, int: nope);
template<class T> int t1() { return _Generic(T, int: 1, long: 2); }
int o = t1<char>();
template<class T> int t2(T v) { return _Generic(v, T: 1, int: 2); }
int p = t2(1);
int q = _Generic(void(), void: 1, default: 2);
