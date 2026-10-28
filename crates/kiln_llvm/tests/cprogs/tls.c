// Thread-local storage through llvm.threadlocal.address.
_Thread_local int tval = 5;
static _Thread_local long tcount;
int next(void) { tcount++; return tval; }
