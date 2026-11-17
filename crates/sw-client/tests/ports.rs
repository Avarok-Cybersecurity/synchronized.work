#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The ports are object-safe and usable as `dyn` on both targets. The loopback below is the
//! smallest adapter that honours the documented contracts; it is a test double, justified
//! because the real adapters need a Citadel node (lanes S2, S4, S5).

use std::collections::{BTreeMap, VecDeque};

use sw_client::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[derive(Default)]
struct Loopback {
    connected: bool,
    queue: VecDeque<Vec<u8>>,
    peers: BTreeMap<String, PeerState>,
    inbox: VecDeque<(String, Vec<u8>)>,
}

impl SessionPort for Loopback {
    fn state(&self) -> SessionState {
        if self.connected {
            SessionState::Connected { cid: 7 }
        } else {
            SessionState::Connecting
        }
    }
    fn send(&mut self, m: Vec<u8>) -> Result<(), SessionError> {
        if !self.connected {
            return Err(SessionError::NotConnected);
        }
        self.queue.push_back(m);
        Ok(())
    }
    fn poll_recv(&mut self) -> Result<Option<Vec<u8>>, SessionError> {
        Ok(self.queue.pop_front())
    }
}

impl P2pPort for Loopback {
    fn peer_state(&self, peer: &str) -> PeerState {
        *self.peers.get(peer).unwrap_or(&PeerState::Unregistered)
    }
    fn register(&mut self, peer: &str) -> Result<(), P2pError> {
        self.peers.insert(peer.into(), PeerState::Registered);
        Ok(())
    }
    fn connect(&mut self, peer: &str) -> Result<(), P2pError> {
        match self.peer_state(peer) {
            PeerState::Unregistered => Err(P2pError::Refused("not registered".into())),
            _ => {
                self.peers.insert(peer.into(), PeerState::Connected);
                Ok(())
            }
        }
    }
    fn deregister(&mut self, peer: &str) -> Result<(), P2pError> {
        self.peers.remove(peer);
        Ok(())
    }
    fn send(&mut self, peer: &str, envelope: Vec<u8>) -> Result<(), P2pError> {
        if self.peer_state(peer) != PeerState::Connected {
            return Err(P2pError::NotConnected);
        }
        self.inbox.push_back((peer.into(), envelope));
        Ok(())
    }
    fn poll_recv(&mut self) -> Result<Option<(String, Vec<u8>)>, P2pError> {
        Ok(self.inbox.pop_front())
    }
}

#[test]
fn session_refuses_sends_until_connected() {
    let mut port = Loopback::default();
    let session: &mut dyn SessionPort = &mut port;
    assert_eq!(session.send(vec![1]), Err(SessionError::NotConnected));
    port.connected = true;
    let session: &mut dyn SessionPort = &mut port;
    session.send(vec![1]).unwrap();
    assert_eq!(session.poll_recv().unwrap(), Some(vec![1]));
    assert_eq!(session.poll_recv().unwrap(), None);
}

#[test]
fn p2p_requires_register_then_connect() {
    let mut port = Loopback::default();
    let p2p: &mut dyn P2pPort = &mut port;
    let peer = "abcdefghijklmnop.laptop@synchronized.work";
    assert_eq!(p2p.send(peer, vec![1]), Err(P2pError::NotConnected));
    assert!(p2p.connect(peer).is_err());
    p2p.register(peer).unwrap();
    p2p.connect(peer).unwrap();
    p2p.send(peer, vec![2]).unwrap();
    assert_eq!(p2p.poll_recv().unwrap(), Some((peer.to_owned(), vec![2])));
    p2p.deregister(peer).unwrap();
    assert_eq!(p2p.peer_state(peer), PeerState::Unregistered);
}
