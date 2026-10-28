// Pointer arithmetic, differences, comparisons and casts.
long diff(int *a, int *b) { return b - a; }
int *step(int *p, long i) { return p + i - 1; }
int deref(int **pp, int i) { return (*pp)[i]; }
int lt(char *a, char *b) { return a < b; }
long addr(void *p) { return (long)p; }
