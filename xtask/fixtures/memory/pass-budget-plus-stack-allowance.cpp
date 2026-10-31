//@ expect: compile-ok
//@ mem<=256
// 768 MiB: over the 256 MiB budget, but within it plus the 1024 MiB stack
// allowance kiln compiles get.
// fake-hog-mib: 768
int x;
