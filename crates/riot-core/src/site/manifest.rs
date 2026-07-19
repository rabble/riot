//! Frozen composite-site manifest v1 data model and strict canonical CBOR codec.
//!
//! The manifest is the owner-signed record at `O:/manifest` that binds three
//! typed namespaces into one site. This module owns the *pure* schema + codec
//! only — deterministic encode, byte-identical decode (`prove_canonical`),
//! definite lengths, strictly-ordered integer keys, a closed failure vocabulary.
//! Signer validation, member classification, and the durable version floor live
//! in sibling modules (`validate`, `version_floor`); the codec never touches
//! willow25 or the store.
//!
//! `role`/`rule`/`display` and the transport allow-list are OPEN enums (a future
//! `community`/`page` is describable by the same schema; unknown values decode to
//! an `Unknown` variant). `layout` and the transport `require` floor are CLOSED
//! enums — an unknown value is a hard reject. No owner-authored render blob is
//! ever parsed (a free-form layout string would be an injection surface).

use minicbor::{Decoder, Encoder};

/// Frozen manifest schema tag (top-level key 0).
pub const SITE_MANIFEST_SCHEMA: &str = "org.riot.site.manifest/1";

/// Largest accepted manifest encoding. Manifests are small binding records, not
/// content; the ceiling keeps a hostile peer from spending unbounded decode work.
pub const MAX_SITE_MANIFEST_BYTES: usize = 16_384;
/// Upper bound on member namespaces bound into one site.
pub const MAX_SITE_MEMBERS: usize = 64;
/// Upper bound on components in `moderation_path`.
pub const MAX_MODERATION_PATH_COMPONENTS: usize = 8;
/// Upper bound on a single path component's byte length.
pub const MAX_PATH_COMPONENT_BYTES: usize = 64;
/// Upper bound on entries in the transport allow-list.
pub const MAX_TRANSPORT_ALLOW: usize = 8;
/// Upper bound on a single declared section name's byte length.
pub const MAX_SECTION_BYTES: usize = 64;
/// Upper bound on the number of declared sections.
pub const MAX_SECTIONS: usize = 64;

/// A namespace bound into the site, with its declared role/rule/display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteMemberV1 {
    /// The member namespace id (32-byte willow namespace key).
    pub ns: [u8; 32],
    pub role: SiteRole,
    pub rule: SiteRule,
    pub display: SiteDisplay,
}

/// The owner-signed site manifest (schema v1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteManifestV1 {
    /// Self-attesting root: must equal the owned namespace owner key hosting it.
    pub root: [u8; 32],
    pub members: Vec<SiteMemberV1>,
    /// Path (as component byte strings) where clients read `/mod/` records.
    pub moderation_path: Vec<Vec<u8>>,
    pub transport_policy: TransportPolicyV1,
    /// Monotonic version; the durable per-root floor refuses any lower value.
    pub version: u64,
    pub layout: SiteLayout,
    /// Named content sections the owner has declared (each a component byte
    /// string, e.g. `b"news"`). Omitted from the wire when empty (canonicity —
    /// see `encode_site_manifest`/`decode_site_manifest`).
    pub sections: Vec<Vec<u8>>,
}

/// Member role. OPEN enum — a future value decodes to `Unknown(raw)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteRole {
    Masthead,
    Comments,
    OpenWire,
    Unknown(u64),
}

/// Member rule *class*. OPEN enum. Invariant 1 binds the two known classes to
/// the namespace key-structure marker bit; an `Unknown` rule cannot be verified
/// against a class and is therefore treated as unverified downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteRule {
    OwnedWrite,
    CommunalOpen,
    Unknown(u64),
}

/// Member display hint. OPEN enum — display-only, never a trust signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteDisplay {
    FrontArticles,
    UnderArticles,
    WireColumn,
    Unknown(u64),
}

/// Resolved section order. CLOSED enum — core resolves it; shells render verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteLayout {
    SiteDefault,
}

/// A transport channel in the allow-list. OPEN enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteTransport {
    Iroh,
    Arti,
    Unknown(u64),
}

/// The mandatory transport floor. CLOSED, ORDERED enum (`none` < `arti`) so the
/// durable require-monotonicity check has a total order; an unknown token fails
/// closed rather than being read as `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequireTransport {
    None,
    Arti,
}

impl RequireTransport {
    /// Strictness rank for the require-monotonicity floor. A higher rank is a
    /// stricter (more private) floor; the floor may never be lowered.
    pub fn strictness(self) -> u8 {
        match self {
            RequireTransport::None => 0,
            RequireTransport::Arti => 1,
        }
    }
}

/// Policy-driven transport: which channels are allowed and the mandatory floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportPolicyV1 {
    pub allow: Vec<SiteTransport>,
    pub require: RequireTransport,
}

/// Stable, closed failure vocabulary for manifest encode/decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteManifestError {
    InputTooLarge,
    TooManyEntries(&'static str),
    FieldTooLarge(&'static str),
    UnknownKey(u64),
    DuplicateOrMisorderedKey(u64),
    MissingKey(u64),
    WrongSchema,
    InvalidEnum(&'static str),
    NonCanonical,
    TrailingBytes,
    Malformed,
}

impl std::fmt::Display for SiteManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SiteManifestError {}

// ---------- open/closed enum code mapping ----------

impl SiteRole {
    fn to_code(self) -> u64 {
        match self {
            SiteRole::Masthead => 0,
            SiteRole::Comments => 1,
            SiteRole::OpenWire => 2,
            SiteRole::Unknown(raw) => raw,
        }
    }
    fn from_code(code: u64) -> Self {
        match code {
            0 => SiteRole::Masthead,
            1 => SiteRole::Comments,
            2 => SiteRole::OpenWire,
            other => SiteRole::Unknown(other),
        }
    }
}

impl SiteRule {
    fn to_code(self) -> u64 {
        match self {
            SiteRule::OwnedWrite => 0,
            SiteRule::CommunalOpen => 1,
            SiteRule::Unknown(raw) => raw,
        }
    }
    fn from_code(code: u64) -> Self {
        match code {
            0 => SiteRule::OwnedWrite,
            1 => SiteRule::CommunalOpen,
            other => SiteRule::Unknown(other),
        }
    }
}

impl SiteDisplay {
    fn to_code(self) -> u64 {
        match self {
            SiteDisplay::FrontArticles => 0,
            SiteDisplay::UnderArticles => 1,
            SiteDisplay::WireColumn => 2,
            SiteDisplay::Unknown(raw) => raw,
        }
    }
    fn from_code(code: u64) -> Self {
        match code {
            0 => SiteDisplay::FrontArticles,
            1 => SiteDisplay::UnderArticles,
            2 => SiteDisplay::WireColumn,
            other => SiteDisplay::Unknown(other),
        }
    }
}

impl SiteTransport {
    fn to_code(self) -> u64 {
        match self {
            SiteTransport::Iroh => 0,
            SiteTransport::Arti => 1,
            SiteTransport::Unknown(raw) => raw,
        }
    }
    fn from_code(code: u64) -> Self {
        match code {
            0 => SiteTransport::Iroh,
            1 => SiteTransport::Arti,
            other => SiteTransport::Unknown(other),
        }
    }
}

impl SiteLayout {
    fn to_code(self) -> u64 {
        match self {
            SiteLayout::SiteDefault => 0,
        }
    }
    fn from_code(code: u64) -> Result<Self, SiteManifestError> {
        match code {
            0 => Ok(SiteLayout::SiteDefault),
            _ => Err(SiteManifestError::InvalidEnum("layout")),
        }
    }
}

impl RequireTransport {
    fn to_code(self) -> u64 {
        match self {
            RequireTransport::None => 0,
            RequireTransport::Arti => 1,
        }
    }
    fn from_code(code: u64) -> Result<Self, SiteManifestError> {
        match code {
            0 => Ok(RequireTransport::None),
            1 => Ok(RequireTransport::Arti),
            _ => Err(SiteManifestError::InvalidEnum("require")),
        }
    }
}

// ---------- encode ----------

pub fn encode_site_manifest(manifest: &SiteManifestV1) -> Result<Vec<u8>, SiteManifestError> {
    validate_structure(manifest)?;
    let pairs = if manifest.sections.is_empty() { 7 } else { 8 };
    encode_bounded(|e| {
        e.map(pairs)?;
        e.u8(0)?.str(SITE_MANIFEST_SCHEMA)?;
        e.u8(1)?.bytes(&manifest.root)?;
        e.u8(2)?.array(manifest.members.len() as u64)?;
        for member in &manifest.members {
            e.map(4)?;
            e.u8(0)?.bytes(&member.ns)?;
            e.u8(1)?.u64(member.role.to_code())?;
            e.u8(2)?.u64(member.rule.to_code())?;
            e.u8(3)?.u64(member.display.to_code())?;
        }
        e.u8(3)?.array(manifest.moderation_path.len() as u64)?;
        for component in &manifest.moderation_path {
            e.bytes(component)?;
        }
        e.u8(4)?.map(2)?;
        e.u8(0)?
            .array(manifest.transport_policy.allow.len() as u64)?;
        for transport in &manifest.transport_policy.allow {
            e.u64(transport.to_code())?;
        }
        e.u8(1)?.u64(manifest.transport_policy.require.to_code())?;
        e.u8(5)?.u64(manifest.version)?;
        e.u8(6)?.u64(manifest.layout.to_code())?;
        if !manifest.sections.is_empty() {
            e.u8(7)?.array(manifest.sections.len() as u64)?;
            for section in &manifest.sections {
                e.bytes(section)?;
            }
        }
        Ok(())
    })
}

// ---------- decode ----------

pub fn decode_site_manifest(input: &[u8]) -> Result<SiteManifestV1, SiteManifestError> {
    check_input_size(input)?;
    let mut d = Decoder::new(input);
    let pairs = definite_map(&mut d)?;
    if pairs > 8 {
        return Err(SiteManifestError::Malformed);
    }

    let mut schema = None;
    let mut root = None;
    let mut members = None;
    let mut moderation_path = None;
    let mut transport_policy = None;
    let mut version = None;
    let mut layout = None;
    // Default empty — key 7 is omitted-when-empty on the wire, so an absent
    // key means `[]`, not a missing field.
    let mut sections: Vec<Vec<u8>> = Vec::new();
    let mut last_key = None;

    for _ in 0..pairs {
        let key = decode_ordered_key(&mut d, &mut last_key)?;
        match key {
            0 => schema = Some(decode_schema(&mut d)?),
            1 => root = Some(decode_id32(&mut d)?),
            2 => members = Some(decode_members(&mut d)?),
            3 => moderation_path = Some(decode_moderation_path(&mut d)?),
            4 => transport_policy = Some(decode_transport_policy(&mut d)?),
            5 => version = Some(decode_u64(&mut d)?),
            6 => layout = Some(SiteLayout::from_code(decode_u64(&mut d)?)?),
            7 => sections = decode_sections(&mut d)?,
            other => return Err(SiteManifestError::UnknownKey(other)),
        }
    }
    finish_input(&d, input)?;
    if schema.as_deref() != Some(SITE_MANIFEST_SCHEMA) {
        return Err(SiteManifestError::WrongSchema);
    }

    let manifest = SiteManifestV1 {
        root: root.ok_or(SiteManifestError::MissingKey(1))?,
        members: members.ok_or(SiteManifestError::MissingKey(2))?,
        moderation_path: moderation_path.ok_or(SiteManifestError::MissingKey(3))?,
        transport_policy: transport_policy.ok_or(SiteManifestError::MissingKey(4))?,
        version: version.ok_or(SiteManifestError::MissingKey(5))?,
        layout: layout.ok_or(SiteManifestError::MissingKey(6))?,
        sections,
    };
    validate_structure(&manifest)?;
    prove_canonical(input, encode_site_manifest(&manifest)?)?;
    Ok(manifest)
}

// ---------- shared validators ----------

/// Whether `section` was declared in `manifest.sections`. The single shared
/// authority used both at article-write time (reject an undeclared section)
/// and at feed-projection time (grouping) — no per-wrapper courtesy check.
pub fn section_is_declared(manifest: &SiteManifestV1, section: &[u8]) -> bool {
    !section.is_empty() && manifest.sections.iter().any(|s| s == section)
}

fn decode_members(d: &mut Decoder<'_>) -> Result<Vec<SiteMemberV1>, SiteManifestError> {
    let len = definite_array(d)?;
    if len as usize > MAX_SITE_MEMBERS {
        return Err(SiteManifestError::TooManyEntries("members"));
    }
    let mut members = Vec::with_capacity(len as usize);
    for _ in 0..len {
        if definite_map(d)? != 4 {
            return Err(SiteManifestError::Malformed);
        }
        let mut ns = None;
        let mut role = None;
        let mut rule = None;
        let mut display = None;
        let mut last_key = None;
        for _ in 0..4 {
            match decode_ordered_key(d, &mut last_key)? {
                0 => ns = Some(decode_id32(d)?),
                1 => role = Some(SiteRole::from_code(decode_u64(d)?)),
                2 => rule = Some(SiteRule::from_code(decode_u64(d)?)),
                3 => display = Some(SiteDisplay::from_code(decode_u64(d)?)),
                other => return Err(SiteManifestError::UnknownKey(other)),
            }
        }
        members.push(SiteMemberV1 {
            ns: ns.ok_or(SiteManifestError::MissingKey(0))?,
            role: role.ok_or(SiteManifestError::MissingKey(1))?,
            rule: rule.ok_or(SiteManifestError::MissingKey(2))?,
            display: display.ok_or(SiteManifestError::MissingKey(3))?,
        });
    }
    Ok(members)
}

fn decode_sections(d: &mut Decoder<'_>) -> Result<Vec<Vec<u8>>, SiteManifestError> {
    let len = definite_array(d)?;
    if len as usize > MAX_SECTIONS {
        return Err(SiteManifestError::TooManyEntries("sections"));
    }
    let mut sections = Vec::with_capacity(len as usize);
    for _ in 0..len {
        let section = d.bytes().map_err(|_| SiteManifestError::Malformed)?;
        if section.is_empty() || section.len() > MAX_SECTION_BYTES {
            return Err(SiteManifestError::FieldTooLarge("section"));
        }
        sections.push(section.to_vec());
    }
    Ok(sections)
}

fn decode_moderation_path(d: &mut Decoder<'_>) -> Result<Vec<Vec<u8>>, SiteManifestError> {
    let len = definite_array(d)?;
    if len as usize > MAX_MODERATION_PATH_COMPONENTS {
        return Err(SiteManifestError::TooManyEntries("moderation_path"));
    }
    let mut components = Vec::with_capacity(len as usize);
    for _ in 0..len {
        let component = d.bytes().map_err(|_| SiteManifestError::Malformed)?;
        if component.len() > MAX_PATH_COMPONENT_BYTES {
            return Err(SiteManifestError::FieldTooLarge("moderation_path"));
        }
        components.push(component.to_vec());
    }
    Ok(components)
}

fn decode_transport_policy(d: &mut Decoder<'_>) -> Result<TransportPolicyV1, SiteManifestError> {
    if definite_map(d)? != 2 {
        return Err(SiteManifestError::Malformed);
    }
    let mut allow = None;
    let mut require = None;
    let mut last_key = None;
    for _ in 0..2 {
        match decode_ordered_key(d, &mut last_key)? {
            0 => {
                let len = definite_array(d)?;
                if len as usize > MAX_TRANSPORT_ALLOW {
                    return Err(SiteManifestError::TooManyEntries("allow"));
                }
                let mut transports = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    transports.push(SiteTransport::from_code(decode_u64(d)?));
                }
                allow = Some(transports);
            }
            1 => require = Some(RequireTransport::from_code(decode_u64(d)?)?),
            other => return Err(SiteManifestError::UnknownKey(other)),
        }
    }
    Ok(TransportPolicyV1 {
        allow: allow.ok_or(SiteManifestError::MissingKey(0))?,
        require: require.ok_or(SiteManifestError::MissingKey(1))?,
    })
}

// ---------- structural validation (bounds only; no semantics) ----------

fn validate_structure(manifest: &SiteManifestV1) -> Result<(), SiteManifestError> {
    if manifest.members.len() > MAX_SITE_MEMBERS {
        return Err(SiteManifestError::TooManyEntries("members"));
    }
    if manifest.moderation_path.len() > MAX_MODERATION_PATH_COMPONENTS {
        return Err(SiteManifestError::TooManyEntries("moderation_path"));
    }
    for component in &manifest.moderation_path {
        if component.len() > MAX_PATH_COMPONENT_BYTES {
            return Err(SiteManifestError::FieldTooLarge("moderation_path"));
        }
    }
    if manifest.transport_policy.allow.len() > MAX_TRANSPORT_ALLOW {
        return Err(SiteManifestError::TooManyEntries("allow"));
    }
    if manifest.sections.len() > MAX_SECTIONS {
        return Err(SiteManifestError::TooManyEntries("sections"));
    }
    for section in &manifest.sections {
        if section.is_empty() || section.len() > MAX_SECTION_BYTES {
            return Err(SiteManifestError::FieldTooLarge("section"));
        }
    }
    Ok(())
}

// ---------- shared codec primitives (mirror newswire/model.rs) ----------

fn encode_bounded<F>(encode: F) -> Result<Vec<u8>, SiteManifestError>
where
    F: FnOnce(
        &mut Encoder<&mut Vec<u8>>,
    ) -> Result<(), minicbor::encode::Error<core::convert::Infallible>>,
{
    let mut buffer = Vec::new();
    let mut encoder = Encoder::new(&mut buffer);
    encode(&mut encoder).map_err(|_| SiteManifestError::Malformed)?;
    if buffer.len() > MAX_SITE_MANIFEST_BYTES {
        return Err(SiteManifestError::InputTooLarge);
    }
    Ok(buffer)
}

fn check_input_size(input: &[u8]) -> Result<(), SiteManifestError> {
    if input.len() > MAX_SITE_MANIFEST_BYTES {
        Err(SiteManifestError::InputTooLarge)
    } else {
        Ok(())
    }
}

fn definite_map(d: &mut Decoder<'_>) -> Result<u64, SiteManifestError> {
    d.map()
        .map_err(|_| SiteManifestError::Malformed)?
        .ok_or(SiteManifestError::NonCanonical)
}

fn definite_array(d: &mut Decoder<'_>) -> Result<u64, SiteManifestError> {
    d.array()
        .map_err(|_| SiteManifestError::Malformed)?
        .ok_or(SiteManifestError::NonCanonical)
}

fn decode_ordered_key(
    d: &mut Decoder<'_>,
    last_key: &mut Option<u64>,
) -> Result<u64, SiteManifestError> {
    let key = d.u64().map_err(|_| SiteManifestError::Malformed)?;
    if last_key.is_some_and(|previous| key <= previous) {
        return Err(SiteManifestError::DuplicateOrMisorderedKey(key));
    }
    *last_key = Some(key);
    Ok(key)
}

fn decode_schema(d: &mut Decoder<'_>) -> Result<String, SiteManifestError> {
    if d.datatype().map_err(|_| SiteManifestError::Malformed)? != minicbor::data::Type::String {
        return Err(SiteManifestError::Malformed);
    }
    let value = d.str().map_err(|_| SiteManifestError::Malformed)?;
    if value.len() > 64 {
        return Err(SiteManifestError::FieldTooLarge("schema"));
    }
    Ok(value.to_string())
}

fn decode_id32(d: &mut Decoder<'_>) -> Result<[u8; 32], SiteManifestError> {
    let bytes = d.bytes().map_err(|_| SiteManifestError::Malformed)?;
    <[u8; 32]>::try_from(bytes).map_err(|_| SiteManifestError::Malformed)
}

fn decode_u64(d: &mut Decoder<'_>) -> Result<u64, SiteManifestError> {
    d.u64().map_err(|_| SiteManifestError::Malformed)
}

fn finish_input(d: &Decoder<'_>, input: &[u8]) -> Result<(), SiteManifestError> {
    if d.position() == input.len() {
        Ok(())
    } else {
        Err(SiteManifestError::TrailingBytes)
    }
}

fn prove_canonical(input: &[u8], encoded: Vec<u8>) -> Result<(), SiteManifestError> {
    if encoded == input {
        Ok(())
    } else {
        Err(SiteManifestError::NonCanonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> SiteManifestV1 {
        SiteManifestV1 {
            root: [7u8; 32],
            members: vec![SiteMemberV1 {
                ns: [7u8; 32],
                role: SiteRole::Masthead,
                rule: SiteRule::OwnedWrite,
                display: SiteDisplay::FrontArticles,
            }],
            moderation_path: vec![b"mod".to_vec()],
            transport_policy: TransportPolicyV1 {
                allow: vec![],
                require: RequireTransport::None,
            },
            version: 1,
            layout: SiteLayout::SiteDefault,
            sections: vec![],
        }
    }

    #[test]
    fn manifest_without_sections_decodes_as_empty_and_round_trips() {
        let m = sample_manifest();
        let bytes = encode_site_manifest(&m).unwrap();
        let decoded = decode_site_manifest(&bytes).unwrap();
        assert!(decoded.sections.is_empty());
        // Re-encoding the decoded value reproduces the exact bytes.
        assert_eq!(encode_site_manifest(&decoded).unwrap(), bytes);
    }

    #[test]
    fn non_empty_sections_round_trip() {
        let mut m = sample_manifest();
        m.sections = vec![b"news".to_vec(), b"analysis".to_vec()];
        let bytes = encode_site_manifest(&m).unwrap();
        assert_eq!(decode_site_manifest(&bytes).unwrap().sections, m.sections);
        assert_eq!(
            encode_site_manifest(&decode_site_manifest(&bytes).unwrap()).unwrap(),
            bytes
        );
    }

    /// Encode top-level keys 0-6 exactly as `encode_site_manifest` does, but
    /// always under a fixed `map(8)` header (the caller appends key 7).
    /// Mirrors the real encoder's key-by-key shape so the only difference
    /// from a genuine encoding is what the caller does with key 7.
    fn encode_manifest_prefix(buffer: &mut Vec<u8>, m: &SiteManifestV1) {
        let mut e = Encoder::new(buffer);
        e.map(8).unwrap();
        e.u8(0).unwrap().str(SITE_MANIFEST_SCHEMA).unwrap();
        e.u8(1).unwrap().bytes(&m.root).unwrap();
        e.u8(2).unwrap().array(m.members.len() as u64).unwrap();
        for member in &m.members {
            e.map(4).unwrap();
            e.u8(0).unwrap().bytes(&member.ns).unwrap();
            e.u8(1).unwrap().u64(member.role.to_code()).unwrap();
            e.u8(2).unwrap().u64(member.rule.to_code()).unwrap();
            e.u8(3).unwrap().u64(member.display.to_code()).unwrap();
        }
        e.u8(3)
            .unwrap()
            .array(m.moderation_path.len() as u64)
            .unwrap();
        for component in &m.moderation_path {
            e.bytes(component).unwrap();
        }
        e.u8(4).unwrap().map(2).unwrap();
        e.u8(0)
            .unwrap()
            .array(m.transport_policy.allow.len() as u64)
            .unwrap();
        for transport in &m.transport_policy.allow {
            e.u64(transport.to_code()).unwrap();
        }
        e.u8(1)
            .unwrap()
            .u64(m.transport_policy.require.to_code())
            .unwrap();
        e.u8(5).unwrap().u64(m.version).unwrap();
        e.u8(6).unwrap().u64(m.layout.to_code()).unwrap();
    }

    /// Hand-encode a manifest carrying key 7 present with a 0-length array —
    /// the shape `encode_site_manifest` never produces (it omits key 7
    /// entirely when `sections` is empty).
    fn encode_manifest_with_forced_empty_sections_key() -> Vec<u8> {
        let m = sample_manifest();
        let mut buffer = Vec::new();
        encode_manifest_prefix(&mut buffer, &m);
        let mut e = Encoder::new(&mut buffer);
        e.u8(7).unwrap().array(0).unwrap();
        buffer
    }

    /// Hand-encode a manifest whose key-7 array header claims `claimed_len`
    /// elements but supplies none — proves the bound is checked before any
    /// allocation/element read is attempted.
    fn encode_manifest_claiming_section_count(claimed_len: u64) -> Vec<u8> {
        let m = sample_manifest();
        let mut buffer = Vec::new();
        encode_manifest_prefix(&mut buffer, &m);
        let mut e = Encoder::new(&mut buffer);
        e.u8(7).unwrap().array(claimed_len).unwrap();
        buffer
    }

    #[test]
    fn key7_present_but_empty_array_is_rejected_non_canonical() {
        let bytes = encode_manifest_with_forced_empty_sections_key();
        assert!(matches!(
            decode_site_manifest(&bytes),
            Err(SiteManifestError::NonCanonical)
        ));
    }

    #[test]
    fn oversize_section_count_rejected_before_alloc() {
        let bytes = encode_manifest_claiming_section_count(MAX_SECTIONS as u64 + 1);
        assert!(matches!(
            decode_site_manifest(&bytes),
            Err(SiteManifestError::TooManyEntries("sections"))
        ));
    }

    #[test]
    fn section_is_declared_accepts_declared_rejects_undeclared() {
        let mut m = sample_manifest();
        m.sections = vec![b"news".to_vec()];
        assert!(section_is_declared(&m, b"news"));
        assert!(!section_is_declared(&m, b"sports"));
        assert!(!section_is_declared(&m, b"")); // empty never declared
    }
}
