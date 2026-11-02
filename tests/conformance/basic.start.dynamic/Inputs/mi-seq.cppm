// The sequence counter of module-init-run.cpp: constant-initialized, so
// it is ready before any dynamic initialization ([basic.start.static]/2).
export module seq;
export int seq_next = 0;
export int seq_take() { return ++seq_next; }
