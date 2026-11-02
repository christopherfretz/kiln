// [class.union.anon], [class.bit]/2, [temp.inst]: the instantiation of a
// class template keeps its anonymous union and struct (the unnamed
// member object and the injected IndirectFields) and its unnamed
// bit-fields. (A function template's block-scope anonymous union is
// covered by function-template-locals.cpp: the dump normalizer has no
// function template specializations yet, spec/ast-dump.md §5.)
//@ expect: ast
template<class T, int N> struct B {
  union { T a; char c; };
  struct { T d; };
  int : N;
  T e : N;
  int : 0;
};
template struct B<short, 2>;
