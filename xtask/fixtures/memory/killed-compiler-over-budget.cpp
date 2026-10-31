//@ expect: compile-ok
//@ mem<=256
// 2 GiB: over the 256 MiB budget plus the 1024 MiB stack allowance.
// fake-hog-mib: 2048
int x;
