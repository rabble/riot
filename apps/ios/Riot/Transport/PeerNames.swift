import Foundation

/// A speakable handle for a phone we can see but have not met.
///
/// **This is a DEVICE handle, not an identity.** Before a sync session opens,
/// Riot knows nothing about who a nearby phone belongs to — there is no key, no
/// signature, nothing to verify. All we have is an ephemeral session nonce the
/// other phone is broadcasting. Naming that honestly is the whole job here: the
/// handle says *"a phone is over there and I can tell it apart from the other
/// phones"*, and it must never imply *"this is Ana."*
///
/// So a peer name is deliberately **not** shaped like a person. A person renders
/// as `Ana · a3f9` — a claimed name bound to a key tag (see
/// `riot_core::profile::resolver::render_display_name`). A phone renders as
/// `Copper Heron` — two words, no separator, no tag. **Never render a peer name
/// with a `·`**, and never let one occupy the place a person's name goes: that
/// collapses the one distinction that matters, between a stranger's device and
/// somebody you have actually met.
///
/// When the sync session opens and the peer's signed entries arrive, we learn
/// who they really are, and the phone becomes the person: `Copper Heron` gives
/// way to `Ana · a3f9`. That transition is the moment a stranger becomes
/// someone you know, and it is worth showing rather than hiding.
///
/// ## Why it's built this way
///
/// The previous generator had four adjectives and four nouns — sixteen possible
/// names, in a room that might hold fifty phones. Worse, it drew both indices
/// linearly from the same nonce (`n % 4` and `(n * 2) % 4`), which correlates
/// them: for a four-word list the noun could only ever land on index 0 or 2, so
/// **half the name space was unreachable**. Everybody was a Blue Kite.
///
/// This version draws from 128 × 128 = 16,384 names and mixes the nonce with a
/// SplitMix64 finalizer before splitting it, so the two halves are independent.
///
/// Collisions are still possible and that is fine — these are handles, not
/// identities, and nothing is authorized on the strength of one. Two Copper
/// Herons in a room is a moment of comedy, not a security failure. (Rough odds
/// of any collision at all: ~1% in a room of 20 phones, ~7% at 50.)
///
/// Word choice is deliberate: calm, concrete, speakable out loud across a room
/// ("the copper heron is yours?"). Nothing violent, nothing cute, nothing that
/// reads as a human first name.
public enum PeerNames {
    static let adjectives: [String] = [
        "Amber", "Ash", "Autumn", "Bay", "Bell", "Birch", "Bitter", "Black",
        "Blue", "Bold", "Brass", "Bright", "Bronze", "Brown", "Calm", "Cedar",
        "Chalk", "Clay", "Clear", "Cloud", "Cold", "Copper", "Coral", "Crisp",
        "Dawn", "Deep", "Dim", "Distant", "Dry", "Dusk", "Dusty", "Early",
        "Earth", "East", "Elder", "Ember", "Even", "Far", "Fern", "First",
        "Flint", "Fog", "Forest", "Free", "Fresh", "Frost", "Gentle", "Glass",
        "Gold", "Grain", "Granite", "Grass", "Gray", "Green", "Half", "Hazel",
        "High", "Hollow", "Honey", "Ink", "Iron", "Ivory", "Jade", "Lake",
        "Lantern", "Late", "Lead", "Light", "Lime", "Linen", "Long", "Low",
        "Marble", "Mild", "Mint", "Mist", "Moss", "Night", "North", "Oak",
        "Olive", "Onyx", "Open", "Orange", "Pale", "Paper", "Patient", "Pearl",
        "Pewter", "Pine", "Plain", "Plum", "Quartz", "Quick", "Quiet", "Rain",
        "Red", "Reed", "River", "Rope", "Rose", "Rough", "Rust", "Salt",
        "Sand", "Sea", "Shale", "Sharp", "Short", "Silent", "Silver", "Slate",
        "Slow", "Small", "Smoke", "Snow", "Soft", "South", "Spring", "Steady",
        "Steel", "Stone", "Storm", "Summer", "Sun", "Swift", "Tall", "Tin",
    ]

    static let nouns: [String] = [
        "Alder", "Anchor", "Anvil", "Arbor", "Arch", "Aspen", "Awl", "Badger",
        "Basin", "Basket", "Beacon", "Bear", "Beech", "Bell", "Bench", "Bird",
        "Bloom", "Boat", "Bridge", "Brook", "Broom", "Burrow", "Canyon", "Cart",
        "Cedar", "Chain", "Chair", "Cliff", "Clover", "Coast", "Comb", "Compass",
        "Cove", "Crane", "Creek", "Crow", "Dam", "Delta", "Dock", "Door",
        "Dove", "Drum", "Dune", "Eagle", "Elm", "Falcon", "Fern", "Ferry",
        "Field", "Finch", "Fjord", "Forge", "Fox", "Garden", "Gate", "Glade",
        "Grove", "Gull", "Hammer", "Harbor", "Hare", "Hawk", "Hearth", "Heron",
        "Hill", "Hollow", "Horn", "Inlet", "Iris", "Ivy", "Jay", "Kestrel",
        "Key", "Kiln", "Kite", "Ladder", "Lamp", "Lantern", "Lark", "Ledge",
        "Lily", "Loom", "Maple", "Marsh", "Meadow", "Mill", "Moth", "Nest",
        "Oak", "Orchard", "Otter", "Owl", "Path", "Pier", "Pine", "Plover",
        "Pond", "Poplar", "Press", "Quarry", "Quill", "Rail", "Raven", "Reed",
        "Ridge", "River", "Robin", "Rope", "Sail", "Shore", "Sparrow", "Spruce",
        "Stair", "Stream", "Swallow", "Swift", "Thistle", "Thrush", "Tide", "Trail",
        "Vale", "Vine", "Wagon", "Well", "Wharf", "Willow", "Window", "Wren",
    ]

    /// Mixes the nonce so the two halves are independent. SplitMix64's finalizer:
    /// cheap, well-distributed, and — unlike the old `n` / `n * 2` pair — it does
    /// not leave most of the name space unreachable.
    static func mix(_ value: UInt64) -> UInt64 {
        var z = value &+ 0x9E37_79B9_7F4A_7C15
        z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
        z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
        return z ^ (z >> 31)
    }

    /// A stable, speakable handle for the phone broadcasting this nonce.
    /// Same nonce → same name, so a peer keeps its name for as long as you can
    /// see it.
    public static func name(sessionNonce: UInt64) -> String {
        let mixed = mix(sessionNonce)
        let adjective = adjectives[Int(mixed % UInt64(adjectives.count))]
        let noun = nouns[Int((mixed >> 32) % UInt64(nouns.count))]
        return "\(adjective) \(noun)"
    }
}
