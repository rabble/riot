//! An always-on seed node: holds a namespace's signed entries and reseeds them
//! to any follower over iroh. v1 is READ-MOSTLY (an origin/mirror seed) — it
//! serves what it holds; ingesting follower publishes (decode + verify + append)
//! is the next slice. State persists so the site identity + ticket are stable
//! across restarts.

use std::fs;
use std::io;
use std::path::Path;

use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::sync::ByteSyncSession;
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, generate_communal_author,
    SignedWillowEntry,
};

use crate::iroh::sync_accept;
use crate::ticket::{mint, Ticket};

/// A seed's durable site: the owner (root) key that mints tickets, the
/// namespace it serves, and the signed entries it reseeds.
pub struct SiteState {
    pub root_key: [u8; 32],
    pub namespace: [u8; 32],
    pub inventory: Vec<SignedWillowEntry>,
}

impl SiteState {
    /// Mint a follow ticket for this site pointing at `seed_node_id`. `none`
    /// floor (public iroh); the owner key signs it so a follower verifies before
    /// dialing.
    pub fn ticket(&self, node_hint: String, epoch: u64, exp: u64) -> Ticket {
        let sk = ed25519_dalek::SigningKey::from_bytes(&self.root_key);
        mint(
            &sk,
            self.namespace,
            "none",
            epoch,
            exp,
            [0u8; 32],
            Some(node_hint),
            None,
        )
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.root_key);
        buf.extend_from_slice(&self.namespace);
        buf.extend_from_slice(&(self.inventory.len() as u32).to_be_bytes());
        for e in &self.inventory {
            put_bytes(&mut buf, &e.entry_bytes);
            put_bytes(&mut buf, &e.capability_bytes);
            buf.extend_from_slice(&e.signature);
            put_bytes(&mut buf, &e.payload_bytes);
        }
        fs::write(path, buf)
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let buf = fs::read(path)?;
        let mut c = Cursor { b: &buf, i: 0 };
        let root_key = c.take32()?;
        let namespace = c.take32()?;
        let count = c.take_u32()?;
        let mut inventory = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let entry_bytes = c.take_bytes()?;
            let capability_bytes = c.take_bytes()?;
            let signature = c.take64()?;
            let payload_bytes = c.take_bytes()?;
            inventory.push(SignedWillowEntry {
                entry_bytes,
                capability_bytes,
                signature,
                payload_bytes,
            });
        }
        Ok(Self {
            root_key,
            namespace,
            inventory,
        })
    }
}

/// Generate a fresh demo site: a communal namespace with `n` signed alert
/// entries and a random owner key. Used to stand a testnet seed up quickly.
pub fn generate_demo_site(n: u8) -> SiteState {
    let author = generate_communal_author().expect("author");
    let namespace = author.identity().namespace_id;
    let inventory = (1..=n).map(|i| demo_entry(&author, i)).collect();
    let root_key = rand32();
    SiteState {
        root_key,
        namespace,
        inventory,
    }
}

/// Accept connections forever, reseeding the current inventory to each follower.
/// Read-mostly: inbound bundles are acknowledged but not ingested in v1.
pub async fn run_seed(endpoint: &iroh::Endpoint, state: &SiteState) {
    loop {
        let session = match ByteSyncSession::new(state.namespace, state.inventory.clone()) {
            Ok(s) => s,
            Err(_) => return,
        };
        // One follower at a time is fine for a v1 testnet seed; a busy seed would
        // spawn a task per accepted connection.
        let _ = sync_accept(endpoint, session, |_| true).await;
    }
}

fn demo_entry(author: &riot_core::willow::EvidenceAuthor, object: u8) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: [object; 16],
        revision_id: [object; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: format!("seed demo entry {object}"),
        description: "riot-seed demo".into(),
        affected_area_claim: None,
        source_claims: vec!["seed".into()],
        ai_assisted: false,
    })
    .unwrap();
    let entry = build_alert_entry(author, &[object; 16], &[object; 16], 1_000, &payload).unwrap();
    let authorised = authorise_entry(author, entry).unwrap();
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }
}

/// 32 random bytes from the OS — an ed25519 SecretKey/SigningKey accepts any
/// 32 bytes as a seed, so no rand_core version dance is needed.
pub fn rand32() -> [u8; 32] {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).expect("os rng");
    b
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn put_bytes(buf: &mut Vec<u8>, b: &[u8]) {
    buf.extend_from_slice(&(b.len() as u32).to_be_bytes());
    buf.extend_from_slice(b);
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}
impl Cursor<'_> {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        if self.i + n > self.b.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated seed state",
            ));
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn take_u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn take32(&mut self) -> io::Result<[u8; 32]> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    fn take64(&mut self) -> io::Result<[u8; 64]> {
        Ok(self.take(64)?.try_into().unwrap())
    }
    fn take_bytes(&mut self) -> io::Result<Vec<u8>> {
        let n = self.take_u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
}
