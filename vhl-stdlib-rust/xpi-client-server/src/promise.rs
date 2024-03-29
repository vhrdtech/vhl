use std::fmt::Debug;
use std::io::Cursor;

use serde::Deserialize;
use tracing::trace;

use xpi::client_server_owned::{EventKind, ReplyKind, RequestId};
use xpi::error::XpiError;

use crate::client::Client;

// TODO: add timeout check
// TODO: handle local error instead of unwraps
#[derive(PartialEq, Default, Debug)]
pub enum Promise<T> {
    #[default]
    None,
    Waiting(RequestId),
    Done(T),
    Err(XpiError),
}

impl<'de, T: Deserialize<'de>> Promise<T> {
    /// Polls the client for new data for this Promise.
    /// Returns true if changes were made (reply or error received).
    pub fn poll(&mut self, client: &mut Client) -> bool {
        if let Promise::Waiting(rid) = self {
            if let Some(ev) = client.poll_one(*rid) {
                // trace!("got promised answer {ev:?}");
                if let EventKind::Reply { result } = ev.kind {
                    match result {
                        Ok(r) => {
                            if let ReplyKind::ReturnValue { data } = r {
                                let len = data.len();
                                let cur = Cursor::new(data);
                                let mut de = rmp_serde::Deserializer::new(cur);
                                match Deserialize::deserialize(&mut de) {
                                    Ok(value) => {
                                        trace!("got promised answer for {:?} ({}B)", ev.seq, len);
                                        *self = Promise::Done(value)
                                    }
                                    Err(e) => {
                                        *self = Promise::Err(XpiError::ClientSideOwned(format!(
                                            "De: {e:?}"
                                        )));
                                    }
                                }
                            } else {
                                *self = Promise::Err(XpiError::Internal);
                            }
                        }
                        Err(e) => {
                            *self = Promise::Err(e.clone());
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn take_if_done(&mut self) -> Option<T> {
        if !matches!(self, Promise::Done(_)) {
            return None;
        }
        let value = core::mem::take(self);
        match value {
            Promise::Done(value) => Some(value),
            _ => None,
        }
    }

    pub fn take_err(&mut self) -> Option<XpiError> {
        if !matches!(self, Promise::Err(_)) {
            return None;
        }
        let value = core::mem::take(self);
        match value {
            Promise::Err(value) => Some(value),
            _ => None,
        }
    }

    /// Returns true if this Promise can be overwritten (None or Err state)
    pub fn is_passive(&self) -> bool {
        match self {
            Promise::None | Promise::Err(_) => true,
            Promise::Done(_) | Promise::Waiting(_) => false,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Promise::None)
    }

    pub fn is_waiting(&self) -> bool {
        matches!(self, Promise::Waiting(_))
    }

    pub fn is_done(&self) -> bool {
        matches!(self, Promise::Done(_))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, Promise::Err(_))
    }

    pub fn as_option(&self) -> Option<&T> {
        match self {
            Promise::Done(v) => Some(v),
            _ => None,
        }
    }
}

// impl<T> Drop for Promise<T> {
//     fn drop(&mut self) {
//         if let Promise::Waiting(rid) = self {
//             warn!("Promise {:?} is being dropped", rid);
//         }
//     }
// }
