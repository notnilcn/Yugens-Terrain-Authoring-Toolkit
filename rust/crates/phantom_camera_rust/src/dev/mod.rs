// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
//
//! Development/harness nodes for the Phantom Camera port.
//!
//! Nothing in this module ports oracle code: the upstream GDScript addon has no
//! counterpart for a telemetry recorder or a probe. These nodes exist so the
//! port source repository's parity harness can observe a live camera rig from
//! *inside* the engine without touching the shipped classes.
//!
//! - [`recorder::PcamRecorder`] — per-frame camera + Phantom Camera JSONL
//!   recorder (the measurement half of the parity harness).
//!
//! `PcamRustProbe` lives in `crate::probe` because it is part of the shipped
//! class surface (the harness asserts against it in editor-less runs too).

pub mod recorder;
