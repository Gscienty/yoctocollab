use std::{
    ptr,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug)]
pub enum Error {
    ExceededMaximumLimit(String),
    LibcFailed(String),
}

const DEFAULT_EPOCH: u64 = 1685290942000;
const DEFAULT_MACHINE_BITS: usize = 10;
const DEFAULT_SEQ_BITS: usize = 12;

pub struct Snowflake {
    epoch: u64,
    machine_id: u64,

    latest_ts: AtomicU64,
    seq: AtomicU64,

    seq_mask: u64,

    time_shift: usize,
    machine_shift: usize,
}

impl Snowflake {
    pub fn new(machine_id: u64) -> Result<Self, Error> {
        let max_machine = (1 << DEFAULT_MACHINE_BITS) - 1;
        let max_seq = (1 << DEFAULT_SEQ_BITS) - 1;

        if machine_id > max_machine {
            return Err(Error::ExceededMaximumLimit(format!(
                "the `machine_id` has exceeded the maximum limit, limit: {max_machine}, machine_id: {machine_id}"
            )));
        }

        Ok(Self {
            epoch: DEFAULT_EPOCH,
            machine_id,

            latest_ts: AtomicU64::new(0),
            seq: AtomicU64::new(0),
            seq_mask: max_seq,

            time_shift: DEFAULT_MACHINE_BITS + DEFAULT_SEQ_BITS,
            machine_shift: DEFAULT_SEQ_BITS,
        })
    }

    #[inline(always)]
    fn current_ts(&self) -> Result<u64, Error> {
        let mut tv = libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };

        match unsafe { libc::gettimeofday(&mut tv, ptr::null_mut()) } {
            0 => Ok(tv.tv_sec as u64 * 1000 + tv.tv_usec as u64 / 1000 - self.epoch),
            errno => Err(Error::LibcFailed(format!(
                "libc::gettimeofday failed, errno: {errno}"
            ))),
        }
    }

    pub fn gen(&mut self) -> Result<u64, Error> {
        loop {
            let now = self.current_ts()?;
            let latest_ts = self.latest_ts.load(Ordering::Relaxed);

            if now < latest_ts {
                continue;
            }

            let seq = self.seq.load(Ordering::Relaxed);
            let next_seq = if now == latest_ts {
                let next_seq = seq + 1;
                if next_seq > self.seq_mask {
                    continue;
                }

                next_seq
            } else {
                0
            };

            if self
                .seq
                .compare_exchange(seq, next_seq, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
                && self
                    .latest_ts
                    .compare_exchange(latest_ts, now, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                break Ok(now << self.time_shift
                    | self.machine_id << self.machine_shift
                    | next_seq);
            }
        }
    }
}

unsafe impl Send for Snowflake {}
