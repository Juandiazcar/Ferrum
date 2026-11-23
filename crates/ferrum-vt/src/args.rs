//! Typed access to the parameters of a control sequence.
//!
//! `vte` already enforces the hardening caps of `docs/10` §3 item 9 (at most 32 parameters,
//! values saturate at `u16::MAX`); this wrapper adds the "missing or zero means default" rule.

use vte::Params;

/// The parameters of a CSI or DCS sequence.
#[derive(Clone, Copy)]
pub(crate) struct Args<'a> {
    params: &'a Params,
}

impl<'a> Args<'a> {
    pub(crate) fn new(params: &'a Params) -> Self {
        Self { params }
    }

    /// Parameter `index` (first sub-parameter), `None` when absent.
    pub(crate) fn raw(self, index: usize) -> Option<u16> {
        self.params.iter().nth(index).and_then(|param| param.first()).copied()
    }

    /// Parameter `index`, or `default` when it is absent or zero (xterm: 0 means "default").
    pub(crate) fn count(self, index: usize, default: usize) -> usize {
        self.raw(index).filter(|&value| value != 0).map_or(default, usize::from)
    }

    /// The parameters with their sub-parameters.
    pub(crate) fn iter(self) -> vte::ParamsIter<'a> {
        self.params.iter()
    }

    /// Parameter `index` with 0 as a real value (erase modes and the like).
    pub(crate) fn mode(self, index: usize) -> u16 {
        self.raw(index).unwrap_or(0)
    }
}
