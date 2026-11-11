//! Experimental environmental soundscape for Horizon Android.
//! Version 0.1 uses dimension and time-of-day automatically. Other scenes
//! can be selected manually until the client exposes authoritative biome
//! and weather state to client mods.
use horizon_mod_sdk::{
    Mod, Registration, game, sound,
    game::{Clock, Player},
};

const INTERVAL_TICKS: u64 = 180;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scene {
    Auto,
    ForestDay,
    ForestNight,
    Rain,
    Cave,
    Beach,
    Underwater,
}

impl Scene {
    fn event(self, player: &Player, clock: Option<&Clock>) -> &'static str {
        match self {
            Self::Auto => automatic_event(player, clock),
            Self::ForestDay => "peugaria_audio:forest.day",
            Self::ForestNight => "peugaria_audio:forest.night",
            Self::Rain => "peugaria_audio:rain",
            Self::Cave => "peugaria_audio:cave",
            Self::Beach => "peugaria_audio:beach",
            Self::Underwater => "peugaria_audio:underwater",
        }
    }
}

fn automatic_event(player: &Player, clock: Option<&Clock>) -> &'static str {
    match player.dimension.as_str() {
        "minecraft:the_nether" => "peugaria_audio:nether",
        "minecraft:the_end" => "peugaria_audio:the_end",
        _ => {
            let time = clock.map(|c| c.day_time % 24_000).unwrap_or(6_000);
            if (13_000..23_000).contains(&time) {
                "peugaria_audio:forest.night"
            } else {
                "peugaria_audio:forest.day"
            }
        }
    }
}

struct EnvironmentalAudio {
    enabled: bool,
    scene: Scene,
    last_play: Option<u64>,
}

impl Mod for EnvironmentalAudio {
    fn init() -> Result<(Self, Registration), String> {
        Ok((
            Self {
                enabled: true,
                scene: Scene::Auto,
                last_play: None,
            },
            Registration::new().command("ambience").on_tick(),
        ))
    }

    fn tick(&mut self, tick: u64) {
        if !self.enabled {
            return;
        }
        if self
            .last_play
            .is_some_and(|last| tick >= last && tick - last < INTERVAL_TICKS)
        {
            return;
        }
        let Some(player) = game::get_player() else {
            self.last_play = None;
            return;
        };
        let clock = game::get_clock();
        let event = self.scene.event(&player, clock.as_ref());
        if let Err(error) = sound::play(event, None, 0.24, 1.0) {
            horizon_mod_sdk::warn(&format!("Could not play {event}: {error}"));
        }
        self.last_play = Some(tick);
    }

    fn command(&mut self, name: &str, args: &str) -> Result<Option<String>, String> {
        if name != "ambience" {
            return Ok(None);
        }
        match args.trim().to_ascii_lowercase().as_str() {
            "off" => {
                self.enabled = false;
                Ok(Some("Environmental audio disabled".into()))
            }
            "on" => {
                self.enabled = true;
                self.last_play = None;
                Ok(Some("Environmental audio enabled".into()))
            }
            "auto" | "" => {
                self.scene = Scene::Auto;
                self.enabled = true;
                self.last_play = None;
                Ok(Some("Scene: automatic (time and dimension)".into()))
            }
            "day" => self.set_scene(Scene::ForestDay, "day forest"),
            "night" => self.set_scene(Scene::ForestNight, "night forest"),
            "rain" => self.set_scene(Scene::Rain, "rain"),
            "cave" => self.set_scene(Scene::Cave, "cave"),
            "beach" => self.set_scene(Scene::Beach, "beach"),
            "underwater" => self.set_scene(Scene::Underwater, "underwater"),
            _ => Ok(Some(
                "Use /ambience auto|on|off|day|night|rain|cave|beach|underwater".into(),
            )),
        }
    }
}

impl EnvironmentalAudio {
    fn set_scene(&mut self, scene: Scene, label: &str) -> Result<Option<String>, String> {
        self.scene = scene;
        self.enabled = true;
        self.last_play = None;
        Ok(Some(format!("Scene: {label}")))
    }
}

horizon_mod_sdk::export_mod!(EnvironmentalAudio);

#[cfg(test)]
mod tests {
    use super::*;

    fn player(dimension: &str) -> Player {
        Player {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            health: 20.0,
            food: 20,
            dimension: dimension.into(),
        }
    }

    #[test]
    fn automatic_scene_tracks_dimension_and_day_time() {
        let day = Clock {
            game_time: 0,
            day_time: 6_000,
        };
        let night = Clock {
            game_time: 0,
            day_time: 18_000,
        };
        assert_eq!(
            automatic_event(&player("minecraft:overworld"), Some(&day)),
            "peugaria_audio:forest.day"
        );
        assert_eq!(
            automatic_event(&player("minecraft:overworld"), Some(&night)),
            "peugaria_audio:forest.night"
        );
        assert_eq!(
            automatic_event(&player("minecraft:the_nether"), None),
            "peugaria_audio:nether"
        );
        assert_eq!(
            automatic_event(&player("minecraft:the_end"), None),
            "peugaria_audio:the_end"
        );
    }
}
