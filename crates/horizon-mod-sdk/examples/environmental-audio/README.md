# PÈUGARIA Environmental Audio

Experimental client-side soundscape mod for Horizon Android.

## Build

Run the audio asset generator, then build from this directory:

    python3 generate_audio.py
    cargo horizon-mod build

The generated package is written to the workspace target/horizon-mod folder.

## Controls

- /ambience auto — choose day/night from the world clock and Nether/End from the dimension.
- /ambience day
- /ambience night
- /ambience rain
- /ambience cave
- /ambience beach
- /ambience underwater
- /ambience off
- /ambience on

## Current limits

Version 0.1 automatically detects only dimension and day/night. Rain, cave,
beach and underwater scenes are manual selections because the current client
mod snapshot does not yet expose authoritative weather, biome or underwater
state. The sound clips are original procedural synthesis, not field recordings.
Playback restarts an eight-second one-shot clip on an eight-second cadence,
with short fades at clip edges. True looping handles and continuous crossfades
are not part of the current sound API.

The sound capability is experimental and must be opted into in the manifest.
All audio assets are generated at build time and are not copied from third-party
recordings.
