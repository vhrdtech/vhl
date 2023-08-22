use std::fmt::Debug;
use std::io::Cursor;

use serde::Deserialize;
use tracing::{trace, warn};

use xpi::client_server_owned::{EventKind, Nrl, ReplyKind, RequestId, RequestKind};
use xpi::error::XpiError;

use crate::client::Client;

// TODO: add timeout check
// TODO: handle local error instead of unwraps
#[derive(PartialEq, Default, Debug)]
pub enum PromiseStream<T> {
    #[default]
    None,
    Waiting {
        rid: RequestId,
        nrl: Nrl,
    },
    Streaming {
        rid: RequestId,
        nrl: Nrl,
        items: Vec<T>,
    },
    Done {
        remaining_items: Vec<T>,
    },
    Err(XpiError),
}

impl<'de, T: Deserialize<'de> + Debug> PromiseStream<T> {
    /// Polls the client for new data for this Promise.
    /// Returns true if changes were made (reply or error received).
    pub fn poll(&mut self, client: &mut Client) -> bool {
        let rid = match self {
            PromiseStream::Waiting { rid, .. } => *rid,
            PromiseStream::Streaming { rid, .. } => *rid,
            _ => {
                return false;
            }
        };
        for ev in client.poll_all(rid) {
            let EventKind::Reply { result } = ev.kind else { continue; };
            let reply_kind = match result {
                Ok(k) => k,
                Err(e) => {
                    *self = PromiseStream::Err(e);
                    return true;
                }
            };
            match reply_kind {
                ReplyKind::StreamOpened => match core::mem::take(self) {
                    PromiseStream::Waiting { nrl, .. } => {
                        *self = PromiseStream::Streaming {
                            rid,
                            nrl,
                            items: Vec::new(),
                        };
                    }
                    PromiseStream::Streaming { rid, nrl, items } => {
                        warn!("PromiseStream: got StreamOpened when already streaming");
                        *self = PromiseStream::Streaming { rid, nrl, items };
                    }
                    PromiseStream::Done { remaining_items } => {
                        warn!("PromiseStream: got StreamOpened in Done state");
                        *self = PromiseStream::Done { remaining_items };
                    }
                    PromiseStream::Err(e) => {
                        *self = PromiseStream::Err(e.clone());
                    }
                    PromiseStream::None => {}
                },
                ReplyKind::StreamUpdate { data } => {
                    let len = data.len();
                    let cur = Cursor::new(data);
                    let mut de = rmp_serde::Deserializer::new(cur);
                    let new_items: Vec<T> = Deserialize::deserialize(&mut de).unwrap();
                    trace!(
                        "got promised stream items for {:?} ({}B {} items)",
                        ev.seq,
                        len,
                        new_items.len()
                    );
                    match core::mem::take(self) {
                        PromiseStream::Streaming {
                            rid,
                            nrl,
                            mut items,
                        } => {
                            items.extend(new_items);
                            *self = PromiseStream::Streaming { rid, nrl, items };
                        }
                        PromiseStream::Waiting { nrl, .. } => {
                            *self = PromiseStream::Streaming {
                                rid,
                                nrl,
                                items: new_items,
                            };
                        }
                        PromiseStream::Done { remaining_items } => {
                            *self = PromiseStream::Done { remaining_items };
                        }
                        PromiseStream::Err(e) => {
                            warn!("PromiseStream: got more items after StreamClosed or Error");
                            *self = PromiseStream::Err(e);
                        }
                        PromiseStream::None => {}
                    }
                }
                ReplyKind::StreamClosed => match core::mem::take(self) {
                    PromiseStream::Streaming { items, .. } => {
                        *self = PromiseStream::Done {
                            remaining_items: items,
                        };
                    }
                    PromiseStream::Waiting { .. } => {
                        *self = PromiseStream::Done {
                            remaining_items: Vec::new(),
                        };
                    }
                    PromiseStream::Done { remaining_items } => {
                        *self = PromiseStream::Done { remaining_items };
                    }
                    PromiseStream::None => {}
                    PromiseStream::Err(e) => {
                        warn!("PromiseStream: got error: {e:?} after stream was already closed");
                        *self = PromiseStream::Err(e);
                    }
                },
                e => warn!("PromiseStream: unexpected event: {e}"),
            }
        }
        false
    }

    pub fn drain(&mut self) -> Vec<T> {
        match core::mem::take(self) {
            PromiseStream::Streaming { rid, nrl, items } => {
                *self = PromiseStream::Streaming {
                    rid,
                    nrl,
                    items: Vec::new(),
                };
                items
            }
            PromiseStream::Done { remaining_items } => {
                *self = PromiseStream::None;
                remaining_items
            }
            _ => Vec::new(),
        }
    }

    pub fn unsubscribe(&mut self, client: &mut Client) {
        match self {
            PromiseStream::Streaming { nrl, .. } | PromiseStream::Waiting { nrl, .. } => {
                let _ = client.send_request(nrl.clone(), RequestKind::CloseStream);
            }
            _ => {}
        }
    }

    /// Returns true if this Promise can be overwritten (None or Err state)
    pub fn is_passive(&self) -> bool {
        match self {
            PromiseStream::None | PromiseStream::Err(_) => true,
            PromiseStream::Waiting { .. } | PromiseStream::Streaming { .. } => false,
            PromiseStream::Done { remaining_items } => remaining_items.is_empty(),
        }
    }
}