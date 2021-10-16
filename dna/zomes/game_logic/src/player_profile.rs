use crate::game_code::get_game_anchor;
use hdk::prelude::*;

pub const PLAYER_LINK_TAG: &str = "PLAYER";

/// This is a Rust structure which represents an actual
/// Holochain entry that stores user's profile for the specific game
/// First we derive just a Rust struct, and then we apply hdk_entry
/// macro to it, which generates code to impelement Holochain entry.
/// id defines how this entry would be called, while visibility defines
/// where an entry will be stored. We plan to store it on DHT, so we
/// go with the "public" value
/// `#[derive(Clone)]` is needed to implement a Rust trait to allow
/// deep copies of the Rust struct, which would come in handy when we
/// want to use.
#[hdk_entry(id = "player_profile", visibility = "public")]
#[derive(Clone)]
pub struct PlayerProfile {
    pub id: AgentPubKey,
    pub nickname: String,
}

/// Struct to receive user input from the UI when user
/// wants to join the game.
/// Note that there are more traits implemented: we need those
/// to be able to send this struct via our zome API
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct JoinGameInfo {
    pub game_code: String,
    pub player_nickname: String,
}

/// Creates a PlayerProfile instance, commits it as a Holochain entry
/// and returns a hash value of this entry
pub fn create_and_hash_entry_player_profile(player_nickname: String) -> ExternResult<EntryHash> {
    // Retrieve info about an agent who is currently executing this code
    // For every instance of the app this would produce different results.
    let player_agent = agent_info()?;
    // Print some debug output into the logs, you'll see it when running
    // integration tests / app in conductor
    // Note the `{:?}` thing: this is what you write when you need to print
    // a Rust struct that implements the Debug trait. For things that implement
    // Display trait (like nickname here of String type) simple `{}` would do.
    debug!(
        "create_and_hash_entry_player_profile | player_nickname: {}, player_agent {:?}",
        player_nickname,
        player_agent.clone()
    );
    // Instantiate a Rust struct to store this data
    let player_profile = PlayerProfile {
        // Beware: this is bad design for real apps, because:
        // 1/ initial_pubkey is linked to app itself, so no roaming profile
        // 2/ lost if app is reinstalled (= that would be basically a new user)
        id: player_agent.agent_initial_pubkey,
        nickname: player_nickname,
    };
    // Commit the Rust struct instance to DHT
    // This is where actual write to DHT happens.
    // Note: this fn isn't idempotent! If someone would try to commit the
    // same player_profile multiple times, every time a Header about entry creation
    // would be created. Since the data is the same, it wouldn't affect it
    // and since our app logic doesn't look for these headers, it wouldn't
    // break the app.
    create_entry(&player_profile)?;
    debug!("create_and_hash_entry_player_profile | profile created, hashing");
    // Calculate a hash value of the entry we just written to DHT:
    // that would be essentially ID of that piece of information.
    // And since there's no ; in the end, this is what we return from current fn
    hash_entry(&player_profile)
}

/// Creates user's profile for the game and registers this user as one of the game players
/// Notice how we packed all input parameters in a single struct: this is a requirement
/// for our function to be exposed as zome API. And even though this particular fn isn't
/// exposed (there's a wrapper for it in lib.rs that is), it's easier for them to have the
/// same signature. Also it's nice to be able to read about all datatypes that cross the API
/// as those would need to be defined as structs.
pub fn join_game(game_info: JoinGameInfo) -> ExternResult<EntryHash> {
    // Another example of logs output with a different priority level
    info!("join_game_with_code | game_info: {:?}", game_info);
    // Retrieve an anchor for the game code provided in input
    let game_anchor = get_game_anchor(game_info.game_code)?;
    debug!("join_game_with_code | anchor created {:?}", &game_anchor);
    // Create player's profile. So far it isn't connected to anything,
    // just a combination of nickname & pub key
    let player_profile_entry_hash =
        create_and_hash_entry_player_profile(game_info.player_nickname)?;
    debug!(
        "join_game_with_code | profile entry hash {:?}",
        &player_profile_entry_hash
    );
    // Create a uni-directional link from the anchor (base) to
    // the player's profile (target) with a tag value of PLAYER_LINK_TAG
    // Having a tag value for the link helps to keep data scheme organized
    create_link(
        game_anchor.clone().into(),
        player_profile_entry_hash.into(),
        LinkTag::new(String::from(PLAYER_LINK_TAG)),
    )?;
    debug!("join_game_with_code | link created");
    // Return entry hash of the anchor wrapped in ExternResult::Ok variant
    Ok(game_anchor)
}

/// Retrieves player profiles that are linked to the anchor for the provided
/// short_unique_code.
pub fn get_game_players(game_code: String) -> ExternResult<Vec<PlayerProfile>> {
    // Retrieve entry hash of our game code anchor
    let game_anchor = get_game_anchor(game_code)?;
    debug!("anchor: {:?}", game_anchor);
    // Retrieve a set of links that have anchor as a base, with the tag PLAYER_LINK_TAG
    let player_links: Links = get_links(
        game_anchor,
        Some(LinkTag::new(String::from(PLAYER_LINK_TAG))),
    )?;
    debug!("links: {:?}", player_links);
    // The following code isn't idiomatic Rust and could've been written
    // in a much more elegant & short way. But, that woudln't have been easy
    // to read for people unfamiliar with Rust, so here we go.
    // First, create a buffer vec for our results. Make it mutable so we
    // can add results one-by-one later
    let mut players = vec![];
    // Iterate through all the links contained inside the link instance
    for link in player_links.into_inner() {
        debug!("link: {:?}", link);
        // Retrieve an element at the hash specified by link.target
        // No fancy retrieve options are applied, so we just go with GetOptions::default()
        let element: Element = get(link.target, GetOptions::default())?
            .ok_or(WasmError::Guest(String::from("Entry not found")))?;
        // Retrieve an Option with our entry inside. Since not all Elements can have
        // entry, their method `entry()` returns an Option which would be None in case
        // the corresponding Element is something different.
        let entry_option = element.entry().to_app_option()?;
        // Now try to unpack the option that we received and write an error to show
        // in case it turns out there's no entry
        let player_profile: PlayerProfile = entry_option.ok_or(WasmError::Guest(
            "The targeted entry is not agent pubkey".into(),
        ))?;
        // Add this PlayerProfile to our results vector
        players.push(player_profile);
    }

    // wrap our vector into ExternResult and return
    Ok(players)
}
