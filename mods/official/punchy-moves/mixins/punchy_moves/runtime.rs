// Punchy Moves animation-only runtime for Horizon.
// The player rig drives third-person model poses. First-person hands and items
// remain owned by the separate Punchy mod.

use super::Game;
use horizon_client::mobs::punchy_moves as rig;
use super::native_hooks::NativeMod;
use std::cell::RefCell;
use std::sync::Mutex;
use serde_json::Value;
use std::time::Instant;

pub(crate) const ID: &str = "punchy_moves";
const NAME: &str = "Punchy Moves";
const VERSION: &str = "1.0.8";
const DESCRIPTION: &str = "Third-person animations with compatibility for external Punchy Moves resource packs. First-person is handled by Punchy.";

const KEY_INSPECT: (&str, &str, &str) = ("inspect", "Inspect held item", "key.keyboard.i");
const KEY_TWIRL_LEFT: (&str, &str, &str) = ("twirl_left", "Elytra twirl left", "key.keyboard.comma");
const KEY_TWIRL_RIGHT: (&str, &str, &str) = ("twirl_right", "Elytra twirl right", "key.keyboard.period");
const KEYS: [(&str, &str, &str); 3] = [KEY_INSPECT, KEY_TWIRL_LEFT, KEY_TWIRL_RIGHT];

struct State {
    last: Option<Instant>,
    inspect: bool,
    twirl: i8,
    resource_pack_attempted: bool,
}
static STATE: Mutex<State> = Mutex::new(State {
    last: None,
    inspect: false,
    twirl: 0,
    resource_pack_attempted: false,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

thread_local! {
    /// Reused per-frame player inputs avoid allocating a new vector every tick.
    static INPUTS: RefCell<Vec<rig::Input>> = const { RefCell::new(Vec::new()) };
}

/// Sample all player motion once per client frame. The native mod switch is
/// the single source of truth for enable/disable; no duplicate config toggle.
pub(crate) fn frame(game: &mut Game<'_>) {
    let now = Instant::now();
    let (dt, inspect, twirl, load_pack) = {
        let mut s = state();
        let dt = s.last.map_or(0.0, |t| now.duration_since(t).as_secs_f32());
        s.last = Some(now);
        let keys = (std::mem::take(&mut s.inspect), s.twirl);
        s.twirl = 0;
        let load_pack = !s.resource_pack_attempted;
        s.resource_pack_attempted = true;
        (dt, keys.0, keys.1, load_pack)
    };

    if load_pack {
        load_external_resource_pack(game);
    }

    if !game.native_on(ID) {
        rig::update(&[], dt);
        return;
    }

    let g: &Game<'_> = game;
    INPUTS.with(|cell| {
        let mut inputs = cell.borrow_mut();
        inputs.clear();
        if let Some(m) = g.player_model.as_ref() {
            let (busy, gliding, swimming) =
                g.player.as_ref().map_or((false, false, false), |p| (p.sleeping, p.fall_flying, p.swimming));
            let mut input = rig::Input::with(m, busy, gliding, swimming);
            input.mining = g.mining.is_some();
            input.inspect = inspect;
            input.twirl = twirl;
            inputs.push(input);
        }
        for m in g.mobs.iter() {
            if &*m.kind.id == "minecraft:player" && g.player_model.as_ref().is_none_or(|p| p.id != m.id) {
                inputs.push(rig::Input::of(m, false));
            }
        }
        rig::update(&inputs, dt);
    });
}

fn value_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for part in path {
        current = current.get(*part)?;
    }
    current.as_str().map(str::to_owned).filter(|name| !name.trim().is_empty())
}


fn value_bool(value: &Value, path: &[&str]) -> bool {
    let mut current = value;
    for part in path {
        let Some(next) = current.get(*part) else { return false };
        current = next;
    }
    current.as_bool().unwrap_or(false)
}

fn first_value_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| value_string(value, path))
}

fn resource_pack_bindings(compat_files: &[(String, Vec<u8>)], clip_names: &std::collections::BTreeSet<String>) -> rig::PackBindings {
    let mut bindings = rig::PackBindings::default();
    let mut files = compat_files.iter().collect::<Vec<_>>();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, bytes) in files {
        let Ok(document) = serde_json::from_slice::<Value>(bytes) else { continue };
        let Some(animations) = document.get("defaults").and_then(|v| v.get("animations")) else { continue };
        let mut candidate = rig::PackBindings {
            idle: value_string(animations, &["idle", "clip"]),
            walk: first_value_string(animations, &[&["walking", "directions", "forward", "clip"], &["walking", "directions", "sprint", "forward", "clip"]]),
            walk_add: value_string(animations, &["walking", "directions", "forward", "add"]),
            sprint_add: value_string(animations, &["walking", "sprint_overlay"]),
            fall: value_string(animations, &["falling", "normal"]),
            free_fall: value_string(animations, &["falling", "free_fall"]),
            jump_landing: first_value_string(animations, &[&["jump", "moving", "landing"], &["jump", "idle", "landing"]]),
            walk_backward: value_string(animations, &["walking", "directions", "backward", "clip"]),
            walk_backward_reverse: value_bool(animations, &["walking", "directions", "backward", "reverse"]),
            walk_left: value_string(animations, &["walking", "directions", "left", "clip"]),
            walk_right: value_string(animations, &["walking", "directions", "right", "clip"]),
            sprint_forward: value_string(animations, &["walking", "directions", "sprint", "forward", "clip"]),
            swim: first_value_string(animations, &[&["swimming", "surface"], &["swimming", "underwater"]]),
            swim_underwater: value_string(animations, &["swimming", "underwater"]),
            elytra: value_string(animations, &["elytra", "normal"]),
            elytra_fast: value_string(animations, &["elytra", "high_speed"]),
            climb: value_string(animations, &["climbing", "clip"]),
            jump: first_value_string(animations, &[&["jump", "moving", "left"], &["jump", "moving", "right"], &["jump", "idle", "left"], &["jump", "idle", "right"]]),
            crouch: value_string(animations, &["crouch", "enter"]),
            attack: value_string(animations, &["attack", "clip"]),
            attack_left: None,
            mining: value_string(animations, &["mining", "clip"]),
            mining_left: None,
        };
        for slot in [
            &mut candidate.idle, &mut candidate.walk, &mut candidate.walk_add,
            &mut candidate.sprint_add, &mut candidate.fall, &mut candidate.free_fall,
            &mut candidate.jump_landing, &mut candidate.walk_backward, &mut candidate.walk_left,
            &mut candidate.walk_right, &mut candidate.sprint_forward, &mut candidate.swim,
            &mut candidate.swim_underwater, &mut candidate.elytra, &mut candidate.elytra_fast, &mut candidate.climb,
            &mut candidate.jump, &mut candidate.crouch, &mut candidate.attack, &mut candidate.mining,
        ] {
            if slot.as_ref().is_some_and(|name| !clip_names.contains(name)) {
                *slot = None;
            }
        }
        bindings = candidate;
    }

    let default = |names: &[&str]| names.iter().find(|name| clip_names.contains(**name)).map(|name| (*name).to_owned());
    bindings.idle = bindings.idle.or_else(|| default(&["idle"]));
    bindings.walk = bindings.walk.or_else(|| default(&["normal_forward", "slow_forward"]));
    bindings.walk_add = bindings.walk_add.or_else(|| default(&["nf_add", "sf_add"]));
    bindings.sprint_add = bindings.sprint_add.or_else(|| default(&["sprint_add"]));
    bindings.fall = bindings.fall.or_else(|| default(&["legacy_review_falling"]));
    bindings.free_fall = bindings.free_fall.or_else(|| default(&["free_fall"]));
    bindings.jump_landing = bindings.jump_landing.or_else(|| default(&["jump-landing"]));
    if bindings.walk_backward.is_none() {
        bindings.walk_backward = default(&["normal_forward"]);
        bindings.walk_backward_reverse = bindings.walk_backward.is_some();
    }
    bindings.walk_left = bindings.walk_left.or_else(|| default(&["sidestep_left"]));
    bindings.walk_right = bindings.walk_right.or_else(|| default(&["sidestep_right"]));
    bindings.sprint_forward = bindings.sprint_forward.or_else(|| default(&["normal_forward"]));
    bindings.swim = bindings.swim.or_else(|| default(&["swimming_overwater", "swimming_underwater"]));
    bindings.swim_underwater = bindings.swim_underwater.or_else(|| default(&["swimming_underwater"]));
    bindings.elytra = bindings.elytra.or_else(|| default(&["elytra_loop"]));
    bindings.elytra_fast = bindings.elytra_fast.or_else(|| default(&["elytra_intense_loop"]));
    bindings.climb = bindings.climb.or_else(|| default(&["climbing_up"]));
    bindings.jump = bindings.jump.or_else(|| default(&["jumping"]));
    bindings.crouch = bindings.crouch.or_else(|| default(&["sneaking"]));
    bindings.attack = bindings.attack.or_else(|| default(&["punch", "punch_right", "sword_attack"]));
    bindings.attack_left = default(&["punch_left", "sword_attack"]);
    bindings.mining = bindings.mining.or_else(|| default(&["mine", "mine_right", "generic_mining_right"]));
    bindings.mining_left = default(&["mine_left", "generic_mining_left"]);
    bindings
}

fn load_external_resource_pack(game: &Game<'_>) {
    let mut files = game.native_data_tree("assets/punchy_moves/animations");
    files.retain(|(path, bytes)| path.ends_with(".animation.json") && !bytes.is_empty());
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut clips = std::collections::BTreeMap::new();
    let mut rejected = 0usize;
    for (_, bytes) in &files {
        if let Ok(parsed) = punchy_engine::load_clips(bytes) {
            for (name, clip) in parsed {
                clips.entry(name).or_insert(clip);
            }
            continue;
        }
        let Ok(document) = serde_json::from_slice::<Value>(bytes) else {
            rejected += 1;
            continue;
        };
        let Some(animations) = document.get("animations").and_then(Value::as_object) else {
            rejected += 1;
            continue;
        };
        for (name, animation) in animations {
            if name.trim().is_empty() || !animation.is_object() || clips.contains_key(name) {
                continue;
            }
            let mut one_animation = serde_json::Map::new();
            one_animation.insert(name.clone(), animation.clone());
            let single = serde_json::json!({
                "format_version": document.get("format_version").cloned().unwrap_or_else(|| Value::String("1.8.0".to_owned())),
                "animations": one_animation
            });
            if let Ok(bytes) = serde_json::to_vec(&single) {
                if let Ok(mut parsed) = punchy_engine::load_clips(&bytes) {
                    if let Some(clip) = parsed.remove(name) {
                        clips.insert(name.clone(), clip);
                    }
                }
            }
        }
    }

    let compat_files = game.native_data_tree("assets/punchy_moves/compat");
    let clip_names = clips.keys().cloned().collect::<std::collections::BTreeSet<_>>();
    let bindings = resource_pack_bindings(&compat_files, &clip_names);
    let loaded = clips.len();
    rig::set_resource_pack_animations(clips, bindings);
    if loaded > 0 {
        log::info!("Punchy Moves: loaded {loaded} third-person animation clips from active resource packs ({rejected} files rejected)");
    } else {
        log::debug!("Punchy Moves: no compatible external animation resource pack active; using native fallback rig");
    }
}

pub(crate) fn resources_reloaded() {
    rig::clear_resource_pack_animations();
    let mut s = state();
    s.resource_pack_attempted = false;
}

pub(crate) fn listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: DESCRIPTION.into(),
        keys: KEYS.iter().map(|(n, l, d)| [(*n).to_owned(), (*l).to_owned(), (*d).to_owned()]).collect(),
    });
}

fn bound(game: &Game<'_>, k: (&str, &str, &str), key: &str) -> bool {
    game.native_mod_key(ID, k.0, k.2) == key
}

/// Animation action keybinds only. Enable/disable is handled by the Mods list.
pub(crate) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_window() || !game.native_playing() || !game.native_on(ID) {
        return;
    }
    if bound(game, KEY_INSPECT, key) {
        state().inspect = true;
    } else if bound(game, KEY_TWIRL_LEFT, key) {
        state().twirl = -1;
    } else if bound(game, KEY_TWIRL_RIGHT, key) {
        state().twirl = 1;
    }
}
