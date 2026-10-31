//@ expect: compile-ok
// 16 GiB: over the default 8192 MiB cap plus the 1024 MiB stack allowance.
// fake-hog-mib: 16384
int x;
