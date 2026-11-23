//! Minting and redeeming a link code, and what a linked account may view.

use sw_model::{Cid, IdFp};
use sw_protocol::gate::Verified;
use sw_protocol::link_code::{canonical, code_hash, link_limits};
use sw_protocol::payload::Link;

use super::{Code, LinkRefusal, ViewerLink, code_key, code_to_bytes, viewer_key, viewer_to_bytes};
use crate::core::receipt::receipt;
use crate::core::{Effect, NodeCore, NodeRefusal};

/// The `typ` both answers name.
const LINK: u8 = 16;

impl NodeCore {
    /// The Person session `cid` acts for when it views `project`: an enrolled Endpoint's
    /// Person for any Project, a linked account's only for the Project it was linked to.
    pub(in crate::core) fn session_person(&self, cid: Cid, project: &str) -> Option<IdFp> {
        if let Some((person, _)) = self.directory.endpoint(cid) {
            return Some(person.clone());
        }
        let linked = self.links.links_project(cid, project);
        linked.then(|| self.links.person_of(cid).cloned()).flatten()
    }

    fn is_member(&self, person: &IdFp, project: &str) -> bool {
        self.tree
            .members(project)
            .is_some_and(|m| m.contains(person))
    }

    /// `LINK_CODE`, signed by an Endpoint of its Person: the code is bound to that Person and
    /// to the Project its session subscribed to last, for `LIFETIME_MS`, once.
    pub(in crate::core) fn link_mint(
        &mut self,
        cid: Cid,
        v: &Verified,
        link: &Link,
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let refuse = |r| NodeRefusal::Link(r);
        let person = v.body.iss.clone();
        let project = self
            .feed
            .latest(cid)
            .ok_or(refuse(LinkRefusal::NoProject))?
            .to_owned();
        if !self.is_member(&person, &project) {
            return Err(refuse(LinkRefusal::NotAMember));
        }
        let code = canonical(&link.code).filter(|c| *c == link.code);
        let hash = code_hash(&code.ok_or(refuse(LinkRefusal::NotACode))?);
        let mut effects = self.prune_codes(now_ms);
        if self.links.codes.contains_key(&hash) {
            return Err(refuse(LinkRefusal::Taken));
        }
        let live = self.links.codes.values();
        let live = live.filter(|c| c.person == person && !c.spent && c.exp > now_ms);
        if live.count() >= link_limits::LIVE_PER_PERSON {
            return Err(refuse(LinkRefusal::TooManyCodes));
        }
        let code = Code {
            person,
            project: project.clone(),
            exp: now_ms + link_limits::LIFETIME_MS,
            spent: false,
        };
        effects.push(Effect::Put {
            key: code_key(&hash),
            value: code_to_bytes(&code)?,
        });
        self.links.codes.insert(hash, code);
        effects.push(receipt(cid, project, v.signal_id, LINK, None)?);
        Ok(effects)
    }

    /// The answer to a refused `LINK_CODE`; nothing was stored and no `seq` burnt.
    pub(in crate::core) fn link_refused(
        &self,
        cid: Cid,
        v: &Verified,
        r: LinkRefusal,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let project = self.feed.latest(cid).unwrap_or_default().to_owned();
        let reason = Some(r.reason().to_owned());
        Ok(vec![receipt(cid, project, v.signal_id, LINK, reason)?])
    }

    /// `LINK_REDEEM`, unsigned, from the App's session (after its `HELLO`): answered with a
    /// receipt whose `id` is the SHA3-256 of the redeem's bytes, naming the Project when stored.
    pub(in crate::core) fn link_redeem(
        &mut self,
        cid: Cid,
        link: &Link,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        if !self.greeted.contains(&cid) {
            return Err(NodeRefusal::NoHello);
        }
        let id = sw_crypto::sha3_256(bytes);
        let (hash, person, project) = match self.redeem_check(cid, link, now_ms) {
            Ok(found) => found,
            Err(r) => {
                let reason = Some(r.reason().to_owned());
                return Ok(vec![receipt(cid, String::new(), id, LINK, reason)?]);
            }
        };
        let (Some(node_id), Some(install_id), Some(acting_username)) =
            (&link.node_id, &link.install_id, &link.acting_username)
        else {
            return Err(NodeRefusal::Malformed);
        };
        let mut effects = Vec::new();
        if let Some(code) = self.links.codes.get_mut(&hash) {
            code.spent = true;
            effects.push(Effect::Put {
                key: code_key(&hash),
                value: code_to_bytes(code)?,
            });
        }
        let viewer = ViewerLink {
            person,
            project: project.clone(),
            node_id: node_id.clone(),
            install_id: install_id.clone(),
            acting_username: acting_username.clone(),
            at: now_ms,
        };
        effects.push(Effect::Put {
            key: viewer_key(cid, node_id),
            value: viewer_to_bytes(&viewer)?,
        });
        self.links.load_viewer(cid, viewer);
        effects.extend(self.prune_codes(now_ms));
        effects.push(receipt(cid, project, id, LINK, None)?);
        Ok(effects)
    }

    /// Every rule a redeem must pass, in order; the attempt counts whatever the outcome.
    fn redeem_check(
        &mut self,
        cid: Cid,
        link: &Link,
        now_ms: u64,
    ) -> Result<([u8; 32], IdFp, String), LinkRefusal> {
        let limits = self.rules.limits;
        if !self.links.take_attempt(cid, now_ms, &limits) {
            return Err(LinkRefusal::TooManyAttempts);
        }
        let hash = code_hash(&canonical(&link.code).ok_or(LinkRefusal::NotACode)?);
        let code = self.links.codes.get(&hash).ok_or(LinkRefusal::Unknown)?;
        if code.spent {
            return Err(LinkRefusal::Spent);
        }
        if code.exp <= now_ms {
            return Err(LinkRefusal::Expired);
        }
        let (person, project) = (code.person.clone(), code.project.clone());
        let enrolled_elsewhere = self
            .directory
            .endpoint(cid)
            .is_some_and(|(p, _)| *p != person);
        let linked_elsewhere = self.links.person_of(cid).is_some_and(|p| *p != person);
        if enrolled_elsewhere || linked_elsewhere {
            return Err(LinkRefusal::OtherPerson);
        }
        if !self.is_member(&person, &project) {
            return Err(LinkRefusal::NotAMember);
        }
        Ok((hash, person, project))
    }

    /// Forgets codes spent or expired more than `KEPT_AFTER_EXPIRY_MS` ago.
    fn prune_codes(&mut self, now_ms: u64) -> Vec<Effect> {
        let keep = link_limits::KEPT_AFTER_EXPIRY_MS;
        let gone: Vec<[u8; 32]> = self
            .links
            .codes
            .iter()
            .filter(|(_, c)| c.exp.saturating_add(keep) <= now_ms)
            .map(|(h, _)| *h)
            .collect();
        gone.iter()
            .map(|h| {
                self.links.codes.remove(h);
                Effect::Delete { key: code_key(h) }
            })
            .collect()
    }

    /// Loads one link key; `Ok(false)` if `key` is not one.
    pub(crate) fn link_load(&mut self, key: &str, value: &[u8]) -> Result<bool, String> {
        let wire = |e: sw_protocol::WireError| format!("{e:?}");
        if let Some(hex) = key.strip_prefix("link/") {
            let code = super::code_from_bytes(value).map_err(wire)?;
            let hash = crate::core::load::hex32(hex).ok_or("bad link code hash")?;
            self.links.load_code(hash, code);
            return Ok(true);
        }
        if let Some(rest) = key.strip_prefix("viewer/") {
            let link = super::viewer_from_bytes(value).map_err(wire)?;
            let cid: Cid = rest
                .split_once('/')
                .and_then(|(c, _)| c.parse().ok())
                .ok_or("bad viewer cid")?;
            if viewer_key(cid, &link.node_id) != key {
                return Err("viewer link stored under another key".into());
            }
            self.links.load_viewer(cid, link);
            return Ok(true);
        }
        Ok(false)
    }
}
