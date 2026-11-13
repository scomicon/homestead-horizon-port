// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

//! Game-independent parts, also compiled as the native bundle's engine crate.
#[path = "core/chain_drive.rs"]
pub mod chain_drive;
#[path = "core/kinetics.rs"]
pub mod kinetics;
#[path = "core/placement.rs"]
pub mod placement;
#[path = "core/pressing.rs"]
pub mod pressing;
#[path = "core/spout.rs"]
pub mod spout;

#[path = "core/depot.rs"]
pub mod depot;

#[path = "core/deployer.rs"]
pub mod deployer;
#[path = "core/sail.rs"]
pub mod sail;

#[path = "core/controls.rs"]
pub mod controls;

#[path = "core/visual.rs"]
pub mod visual;

#[path = "core/waterwheel.rs"]
pub mod waterwheel;

#[path = "core/milling.rs"]
pub mod milling;

#[path = "core/crushing.rs"]
pub mod crushing;

#[path = "core/crafter.rs"]
pub mod crafter;

#[path = "core/vault.rs"]
pub mod vault;

#[path = "core/fan.rs"]
pub mod fan;

#[path = "core/fan_entities.rs"]
pub mod fan_entities;

#[path = "core/particles.rs"]
pub mod particles;

#[path = "core/source_trig.rs"]
pub mod source_trig;

#[path = "core/funnel.rs"]
pub mod funnel;
#[path = "core/transport.rs"]
pub mod transport;

#[path = "core/chute.rs"]
pub mod chute;

#[path = "core/drilling.rs"]
pub mod drilling;
#[path = "core/saw_processing.rs"]
pub mod saw_processing;

#[path = "core/sequenced.rs"]
pub mod sequenced;

#[path = "core/audio.rs"]
pub mod audio;

#[path = "core/basin.rs"]
pub mod basin;

#[path = "core/fluid_transport.rs"]
pub mod fluid_transport;

#[path = "core/burner.rs"]
pub mod burner;

#[path = "core/burner_visual.rs"]
pub mod burner_visual;

#[path = "core/contraption/mod.rs"]
pub mod contraption;

// Wave 1 subsystem cores (pure logic, unit-tested; adapters follow).
#[path = "core/arm.rs"]
pub mod arm;
#[path = "core/pipes.rs"]
pub mod pipes;
#[path = "core/tank_multiblock.rs"]
pub mod tank_multiblock;
#[path = "core/boiler.rs"]
pub mod boiler;
#[path = "core/steam_engine.rs"]
pub mod steam_engine;
#[path = "core/steam_whistle.rs"]
pub mod steam_whistle;
#[path = "core/train_track.rs"]
pub mod train_track;
#[path = "core/train_graph.rs"]
pub mod train_graph;
#[path = "core/train_motion.rs"]
pub mod train_motion;
#[path = "core/train_signals.rs"]
pub mod train_signals;
#[path = "core/train_schedule.rs"]
pub mod train_schedule;
#[path = "core/redstone_link.rs"]
pub mod redstone_link;
#[path = "core/redstone_switches.rs"]
pub mod redstone_switches;
#[path = "core/nixie.rs"]
pub mod nixie;
#[path = "core/display_board.rs"]
pub mod display_board;
#[path = "core/display_link.rs"]
pub mod display_link;
#[path = "core/package.rs"]
pub mod package;
#[path = "core/packager.rs"]
pub mod packager;
#[path = "core/stock_network.rs"]
pub mod stock_network;
#[path = "core/factory_gauge.rs"]
pub mod factory_gauge;
#[path = "core/package_post.rs"]
pub mod package_post;
#[path = "core/material_checklist.rs"]
pub mod material_checklist;
#[path = "core/schematic_printer.rs"]
pub mod schematic_printer;
#[path = "core/schematic_tool.rs"]
pub mod schematic_tool;
#[path = "core/schematic_table.rs"]
pub mod schematic_table;
#[path = "core/schematicannon.rs"]
pub mod schematicannon;
