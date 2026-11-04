// [dcl.type.class] (clang extension, nullability on classes): the mark
// after a class-key applies only to a non-union class, and only a mark on
// some redeclaration of the class (for a class template specialization:
// of its primary template) lets the class type carry a nullability
// qualifier. A mark on a partial or explicit specialization does not
// count, nor does a class template's injected-class-name (clang 19).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:7 error "'_Nullable' attribute only applies to classes"
//@ diag: 22:7 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'Plain'"
//@ diag: 26:10 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'Y<int *>'"
//@ diag: 28:8 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'Y<int>'"
//@ diag: 31:5 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'Z<T>'"
//@ diag: 32:8 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'Z<T>'"
//@ diag: 34:13 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'Late'"
//@ diag: 35:5 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'int'"
template <class T> class Z;
class Late;

union _Nullable U { int a; };
class Plain {};
Plain _Nullable plain;

template <class T> class Y {};
template <class T> class _Nullable Y<T *> {};
Y<int *> _Nullable y1;
template <> class _Nullable Y<int> {};
Y<int> _Nullable y2;

template <class T> class _Nullable Z {
  Z _Nullable self();
  Z<T> _Nullable other();
};
extern Late _Nullable late;
int _Nullable i;
class _Nullable Late {};
