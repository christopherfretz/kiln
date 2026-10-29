//@ expect: ast
//@ oracle: clang
// [temp.spec.partial]/1, [temp.expl.spec]/1: variable template partial and
// explicit specializations.
template <class T> int rank_v = 0;
template <class T> int rank_v<T *> = 1;
template <> int rank_v<void> = -1;
int a = rank_v<int **>;
int b = rank_v<void>;
