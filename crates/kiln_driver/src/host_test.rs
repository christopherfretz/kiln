//! The test host-call handler of the hidden `-fkiln-host-call-test-handler`
//! (spec/host-calls.md §7, WP-1990): kiln's own tests of
//! `__builtin_kiln_host_call` through the driver. Not a supported
//! interface.

use kiln_parse::{HostCalls, HostError, HostOut};

use crate::args::{Action, Args, Input};

/// The test handler: op 0 counts the calls before it, op 1 is a counter,
/// op 2 echoes its inputs, op 3 fails, op 4 returns its capacities, op 5
/// arms the cancellation flag that op 6 then fails on.
#[derive(Debug, Default)]
pub(crate) struct TestHandler {
    calls: u64,
    counter: u64,
    cancelled: bool,
}

impl HostCalls for TestHandler {
    fn call(
        &mut self,
        op: u32,
        ints: &[u64],
        bytes: &[u8],
        out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        let before = self.calls;
        self.calls += 1;
        match op {
            0 => Ok(before),
            1 => {
                let v = self.counter;
                self.counter += 1;
                Ok(v)
            }
            2 => {
                out.write_ints(ints);
                out.write_bytes(bytes);
                Ok((ints.len() as u64) << 32 | bytes.len() as u64)
            }
            3 => Err(HostError::new("boom")),
            4 => Ok((out.int_capacity() as u64) << 32 | out.byte_capacity() as u64),
            5 => {
                self.cancelled = true;
                Ok(0)
            }
            6 if self.cancelled => Err(HostError::new("cancelled")),
            6 => Ok(0),
            _ => Err(HostError::new(format!("unknown op {op}"))),
        }
    }
}

/// The handler `args` registers, if any.
pub(crate) fn handler(args: &Args) -> Option<Box<dyn HostCalls + Send>> {
    args.host_call_test_handler
        .then(|| Box::new(TestHandler::default()) as Box<dyn HostCalls + Send>)
}

/// The output of this compile that would outlive it, which a host-call
/// handler may not be used for (spec/host-calls.md §6): a module
/// interface or a precompiled header.
pub(crate) fn artifact(args: &Args, inputs: &[&Input]) -> Option<&'static str> {
    if args.action == Action::Precompile || args.precompile || args.module_header.is_some() {
        return Some("a module file");
    }
    if args.module_output.is_some() {
        return Some("a module file (-fmodule-output)");
    }
    inputs.iter().any(|i| crate::pch::is_producer(args, i)).then_some("a precompiled header")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(h: &mut TestHandler, op: u32, ints: &[u64], bytes: &[u8]) -> Result<u64, HostError> {
        let (mut io, mut bo) = (Vec::new(), Vec::new());
        let mut out = HostOut::new(&mut io, &mut bo, 3, 5);
        h.call(op, ints, bytes, &mut out)
    }

    #[test]
    fn ops() {
        let mut h = TestHandler::default();
        assert_eq!(call(&mut h, 1, &[], b""), Ok(0));
        assert_eq!(call(&mut h, 1, &[], b""), Ok(1));
        assert_eq!(call(&mut h, 0, &[], b""), Ok(2));
        assert_eq!(call(&mut h, 2, &[7, 8], b"abc"), Ok(2 << 32 | 3));
        assert_eq!(call(&mut h, 3, &[], b"").map_err(|e| e.to_string()), Err("boom".into()));
        assert_eq!(call(&mut h, 4, &[], b""), Ok(3 << 32 | 5));
        assert_eq!(call(&mut h, 6, &[], b""), Ok(0));
        assert_eq!(call(&mut h, 5, &[], b""), Ok(0));
        assert_eq!(call(&mut h, 6, &[], b"").map_err(|e| e.to_string()), Err("cancelled".into()));
        assert_eq!(
            call(&mut h, 9, &[], b"").map_err(|e| e.to_string()),
            Err("unknown op 9".into())
        );
    }
}
