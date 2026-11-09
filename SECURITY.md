# Security policy

## Reporting a vulnerability

Please report security issues privately through GitHub's private
vulnerability reporting: on the
[Security tab](https://github.com/christopherfretz/kiln/security), choose
**Report a vulnerability**
([direct link](https://github.com/christopherfretz/kiln/security/advisories/new)).
Do not open a public issue for a security problem.

Include the kiln version (`kiln++ --version`), the command line, and the
input that triggers the problem. If kiln crashed, include its crash
reproducer as well ([docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md)
explains what it writes). We will acknowledge the report, agree a fix and
a disclosure date with you, and credit you in the advisory unless you
prefer otherwise.

## Supported versions

Security fixes go into the latest release of the current major version
([docs/STABILITY.md](docs/STABILITY.md)). Before 1.0 only `main` is
supported.

## Scope

kiln is a compiler. It runs with the privileges of whoever invokes it and
trusts its command line and its environment. A security issue is one
where **untrusted source code** makes kiln do something it should not, as
it would in a service that compiles code submitted by others:

- **Compiler crashes on untrusted input.** Any internal compiler error,
  panic, abort or hang caused by a source file. kiln is
  `#![forbid(unsafe_code)]` throughout, so memory corruption in kiln
  itself would be a serious bug. Report any sign of it.
- **Oven resource exhaustion.** Constant evaluation that escapes its
  budgets: evaluation that runs past `-fconstexpr-steps`, uses more live
  memory than `-fconstexpr-memory`, or recurses past `-fconstexpr-depth`
  without the error that should stop it, or that exhausts kiln's stack
  instead of producing the `stack exhausted` diagnostic.
- Template instantiation or preprocessing that consumes unbounded memory
  or time despite `-ftemplate-depth` and the other limits.

Out of scope:

- The behaviour of the **compiled program**. Code that kiln compiles
  correctly does what its source says. A miscompilation is a correctness
  bug, so file it as a normal issue, unless it defeats a security
  mechanism such as a sanitizer, `-fstack-protector` or branch protection.
- Anything that needs control of kiln's command line, environment
  variables, response files or include paths, or of the LLVM tools it
  runs. Those are trusted inputs. For example, `-fconstexpr-steps=` set
  very high is not a vulnerability.
- Bugs in LLVM, the system linker or the standard library. Report those
  to their projects.
- Ordinary crashes on code you wrote yourself. Those are welcome as
  [public issues](https://github.com/christopherfretz/kiln/issues).
