use std::os::raw::c_uint;
use std::{fmt, mem};

use crate::bindings as C;
use crate::utils::{c_uint_to_u32, u32_as_c_uint};

#[repr(transparent)]
pub struct WorkCompletion(C::ibv_wc);

impl WorkCompletion {
    #[inline]
    pub fn status(&self) -> Result<(), WorkCompletionError> {
        if self.0.status == C::IBV_WC_SUCCESS {
            Ok(())
        } else {
            Err(WorkCompletionError::from_c_uint(self.0.status))
        }
    }

    #[inline]
    #[must_use]
    pub fn wr_id(&self) -> u64 {
        self.0.wr_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum WorkCompletionError {
    LocalLength = c_uint_to_u32(C::IBV_WC_LOC_LEN_ERR),
    LocalQPOperation = c_uint_to_u32(C::IBV_WC_LOC_QP_OP_ERR),
    LocalEEContextOperation = c_uint_to_u32(C::IBV_WC_LOC_EEC_OP_ERR),
    LocalProtection = c_uint_to_u32(C::IBV_WC_LOC_PROT_ERR),
    WRFlush = c_uint_to_u32(C::IBV_WC_WR_FLUSH_ERR),
    MWBind = c_uint_to_u32(C::IBV_WC_MW_BIND_ERR),
    BadResponse = c_uint_to_u32(C::IBV_WC_BAD_RESP_ERR),
    LocalAccess = c_uint_to_u32(C::IBV_WC_LOC_ACCESS_ERR),
    RemoteInvalidRequest = c_uint_to_u32(C::IBV_WC_REM_INV_REQ_ERR),
    RemoteAccess = c_uint_to_u32(C::IBV_WC_REM_ACCESS_ERR),
    RemoteOperation = c_uint_to_u32(C::IBV_WC_REM_OP_ERR),
    RetryExceeded = c_uint_to_u32(C::IBV_WC_RETRY_EXC_ERR),
    RnrRetryExceeded = c_uint_to_u32(C::IBV_WC_RNR_RETRY_EXC_ERR),
    LocalRDDViolation = c_uint_to_u32(C::IBV_WC_LOC_RDD_VIOL_ERR),
    RemoteInvalidRDRequest = c_uint_to_u32(C::IBV_WC_REM_INV_RD_REQ_ERR),
    RemoteAborted = c_uint_to_u32(C::IBV_WC_REM_ABORT_ERR),
    InvalidEEContextNumber = c_uint_to_u32(C::IBV_WC_INV_EECN_ERR),
    InvalidEEContextState = c_uint_to_u32(C::IBV_WC_INV_EEC_STATE_ERR),
    Fatal = c_uint_to_u32(C::IBV_WC_FATAL_ERR),
    ResponseTimeout = c_uint_to_u32(C::IBV_WC_RESP_TIMEOUT_ERR),
    General = c_uint_to_u32(C::IBV_WC_GENERAL_ERR),
    TagMatching = c_uint_to_u32(C::IBV_WC_TM_ERR),
    TagMatchingRndvIncomplete = c_uint_to_u32(C::IBV_WC_TM_RNDV_INCOMPLETE),
}

impl WorkCompletionError {
    fn to_c_uint(self) -> c_uint {
        #[allow(clippy::as_conversions)]
        u32_as_c_uint(self as u32)
    }

    #[allow(clippy::as_conversions)]
    fn from_c_uint(val: c_uint) -> Self {
        let last = Self::TagMatchingRndvIncomplete.to_c_uint();
        assert!((1..=last).contains(&val), "unknown work completion status");
        // SAFETY: continuous integer enum
        unsafe { mem::transmute(val as u32) }
    }
}

impl fmt::Display for WorkCompletionError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f) // TODO: error message
    }
}

impl std::error::Error for WorkCompletionError {}

#[cfg(test)]
mod tests {
    use numeric_cast::NumericCast;

    use super::*;

    #[test]
    fn continuous() {
        let err = [
            WorkCompletionError::LocalLength,
            WorkCompletionError::LocalQPOperation,
            WorkCompletionError::LocalEEContextOperation,
            WorkCompletionError::LocalProtection,
            WorkCompletionError::WRFlush,
            WorkCompletionError::MWBind,
            WorkCompletionError::BadResponse,
            WorkCompletionError::LocalAccess,
            WorkCompletionError::RemoteInvalidRequest,
            WorkCompletionError::RemoteAccess,
            WorkCompletionError::RemoteOperation,
            WorkCompletionError::RetryExceeded,
            WorkCompletionError::RnrRetryExceeded,
            WorkCompletionError::LocalRDDViolation,
            WorkCompletionError::RemoteInvalidRDRequest,
            WorkCompletionError::RemoteAborted,
            WorkCompletionError::InvalidEEContextNumber,
            WorkCompletionError::InvalidEEContextState,
            WorkCompletionError::Fatal,
            WorkCompletionError::ResponseTimeout,
            WorkCompletionError::General,
            WorkCompletionError::TagMatching,
            WorkCompletionError::TagMatchingRndvIncomplete,
        ];

        let mut numbers = err.iter().map(|e| e.to_c_uint()).collect::<Vec<_>>();
        numbers.sort_unstable();

        assert_eq!(numbers.first().copied().unwrap(), 1);

        assert_eq!(
            numbers.last().copied().unwrap(),
            numbers.len().numeric_cast::<c_uint>()
        );
    }
}
