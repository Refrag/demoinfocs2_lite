// based on https://github.com/unicbm/demoparser/blob/aad2e50d16ffc21557e6780dffeb60face300f9b/src/parser/src/second_pass/usercmd_delta.rs
//! Stateful decoding for `CMsgServerUserCmd`.
//!
//! Singular fields retain protobuf wire encoding except for wire type 7,
//! which resets a field to its declared default. The repeated input-history
//! and subtick fields use the replacement-list encoding observed in current
//! CS2 demos. Unknown or malformed operations fail the whole delta so callers
//! can keep the previous per-player baseline unchanged.

use std::fmt;

use foldhash::HashMap;
use prost::Message;

use crate::protobuf::{
    CBaseUserCmdExecutionNotes, CInButtonStatePb, CMsgQAngle, CMsgServerUserCmd,
    CsgoUserCmdPb,
};

#[derive(Clone, PartialEq, Message)]
struct DeltaBaseUserCmdPb {
    #[prost(int32, optional, tag = "1")]
    legacy_command_number: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    client_tick: Option<i32>,
    #[prost(uint32, optional, tag = "17")]
    prediction_offset_ticks_x256: Option<u32>,
    #[prost(message, optional, tag = "3")]
    buttons_pb: Option<CInButtonStatePb>,
    #[prost(message, optional, tag = "4")]
    viewangles: Option<CMsgQAngle>,
    #[prost(float, optional, tag = "5")]
    forwardmove: Option<f32>,
    #[prost(float, optional, tag = "6")]
    leftmove: Option<f32>,
    #[prost(float, optional, tag = "7")]
    upmove: Option<f32>,
    #[prost(int32, optional, tag = "8")]
    impulse: Option<i32>,
    #[prost(int32, optional, tag = "9")]
    weaponselect: Option<i32>,
    #[prost(int32, optional, tag = "10")]
    random_seed: Option<i32>,
    #[prost(int32, optional, tag = "11")]
    mousedx: Option<i32>,
    #[prost(int32, optional, tag = "12")]
    mousedy: Option<i32>,
    #[prost(uint32, optional, tag = "14")]
    pawn_entity_handle: Option<u32>,
    #[prost(bytes = "bytes", repeated, tag = "18")]
    subtick_moves_delta: Vec<prost::bytes::Bytes>,
    #[prost(bytes = "bytes", optional, tag = "19")]
    move_crc: Option<prost::bytes::Bytes>,
    #[prost(uint32, optional, tag = "20")]
    consumed_server_angle_changes: Option<u32>,
    #[prost(int32, optional, tag = "21")]
    cmd_flags: Option<i32>,
    #[prost(bytes = "bytes", optional, tag = "22")]
    execution_notes: Option<prost::bytes::Bytes>,
}

#[derive(Clone, PartialEq, Message)]
struct DeltaCsgoUserCmdPb {
    #[prost(message, optional, tag = "1")]
    base: Option<DeltaBaseUserCmdPb>,
    #[prost(bytes = "bytes", repeated, tag = "2")]
    input_history_delta: Vec<prost::bytes::Bytes>,
    #[prost(int32, optional, tag = "6")]
    attack1_start_history_index: Option<i32>,
    #[prost(int32, optional, tag = "7")]
    attack2_start_history_index: Option<i32>,
    #[prost(bool, optional, tag = "9")]
    left_hand_desired: Option<bool>,
    #[prost(bool, optional, tag = "11")]
    is_predicting_body_shot_fx: Option<bool>,
    #[prost(bool, optional, tag = "12")]
    is_predicting_head_shot_fx: Option<bool>,
    #[prost(bool, optional, tag = "13")]
    is_predicting_kill_ragdolls: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageSchema {
    CsgoUserCmd,
    BaseUserCmd,
    Buttons,
    QAngle,
    Vector,
    InputHistory,
    Interpolation,
    InterpolationCl,
    SubtickMove,
    ExecutionNotes,
}

impl MessageSchema {
    fn field_wire_type(self, field: u64) -> Option<u8> {
        match self {
            Self::CsgoUserCmd => match field {
                1 | 2 => Some(2),
                6 | 7 | 9 | 11 | 12 | 13 => Some(0),
                _ => None,
            },
            Self::BaseUserCmd => match field {
                1 | 2 | 8 | 9 | 10 | 11 | 12 | 14 | 17 | 20 | 21 => Some(0),
                3 | 4 | 18 | 19 | 22 => Some(2),
                5..=7 => Some(5),
                _ => None,
            },
            Self::Buttons => match field {
                1..=3 => Some(0),
                _ => None,
            },
            Self::QAngle => match field {
                1..=3 => Some(5),
                _ => None,
            },
            Self::Vector => match field {
                1..=4 => Some(5),
                _ => None,
            },
            Self::InputHistory => match field {
                2 | 12..=15 | 66..=69 => Some(2),
                4 | 6 | 64 | 65 => Some(0),
                5 | 7 => Some(5),
                _ => None,
            },
            Self::Interpolation => match field {
                1 | 2 => Some(0),
                3 => Some(5),
                _ => None,
            },
            Self::InterpolationCl => match field {
                3 => Some(5),
                _ => None,
            },
            Self::SubtickMove => match field {
                1 | 2 => Some(0),
                3 | 4 | 5 | 8 | 9 => Some(5),
                _ => None,
            },
            Self::ExecutionNotes => match field {
                1 => Some(2),
                _ => None,
            },
        }
    }

    fn child(self, field: u64) -> Option<Self> {
        match (self, field) {
            (Self::CsgoUserCmd, 1) => Some(Self::BaseUserCmd),
            (Self::BaseUserCmd, 3) => Some(Self::Buttons),
            (Self::BaseUserCmd, 4) => Some(Self::QAngle),
            (Self::BaseUserCmd, 22) => Some(Self::ExecutionNotes),
            (Self::InputHistory, 2 | 69) => Some(Self::QAngle),
            (Self::InputHistory, 12) => Some(Self::InterpolationCl),
            (Self::InputHistory, 13..=15) => Some(Self::Interpolation),
            (Self::InputHistory, 66..=68) => Some(Self::Vector),
            _ => None,
        }
    }

    fn reset_fields(self) -> &'static [(u64, u8)] {
        match self {
            Self::CsgoUserCmd => &[
                (1, 2),
                (2, 2),
                (6, 0),
                (7, 0),
                (9, 0),
                (11, 0),
                (12, 0),
                (13, 0),
            ],
            Self::BaseUserCmd => &[
                (1, 0),
                (2, 0),
                (3, 2),
                (4, 2),
                (5, 5),
                (6, 5),
                (7, 5),
                (8, 0),
                (9, 0),
                (10, 0),
                (11, 0),
                (12, 0),
                (14, 0),
                (17, 0),
                (18, 2),
                (19, 2),
                (20, 0),
                (21, 0),
                (22, 2),
            ],
            Self::Buttons => &[(1, 0), (2, 0), (3, 0)],
            Self::QAngle => &[(1, 5), (2, 5), (3, 5)],
            Self::Vector => &[(1, 5), (2, 5), (3, 5), (4, 5)],
            Self::InputHistory => &[
                (2, 2),
                (4, 0),
                (5, 5),
                (6, 0),
                (7, 5),
                (12, 2),
                (13, 2),
                (14, 2),
                (15, 2),
                (64, 0),
                (65, 0),
                (66, 2),
                (67, 2),
                (68, 2),
                (69, 2),
            ],
            Self::Interpolation => &[(1, 0), (2, 0), (3, 5)],
            Self::InterpolationCl => &[(3, 5)],
            Self::SubtickMove => {
                &[(1, 0), (2, 0), (3, 5), (4, 5), (5, 5), (8, 5), (9, 5)]
            }
            Self::ExecutionNotes => &[(1, 2)],
        }
    }

    fn write_default(self, field: u64, wire_type: u8, out: &mut Vec<u8>) -> Option<()> {
        match wire_type {
            0 => {
                let value = match (self, field) {
                    (Self::CsgoUserCmd, 6 | 7) | (Self::InputHistory, 65) | (Self::Interpolation, 1 | 2) => u64::MAX,
                    (Self::BaseUserCmd, 14) => 0x00ff_ffff,
                    _ => 0,
                };
                write_varint(value, out);
            }
            1 => out.extend_from_slice(&[0; 8]),
            2 => {
                let nested = self.child(field).map_or_else(Vec::new, MessageSchema::explicit_defaults);
                write_varint(nested.len() as u64, out);
                out.extend_from_slice(&nested);
            }
            5 => out.extend_from_slice(&[0; 4]),
            _ => return None,
        }
        Some(())
    }

    fn explicit_defaults(self) -> Vec<u8> {
        let mut out = Vec::new();
        for &(field, wire_type) in self.reset_fields() {
            write_varint((field << 3) | u64::from(wire_type), &mut out);
            self.write_default(field, wire_type, &mut out).expect("schema contains only protobuf wire types");
        }
        out
    }
}

fn read_varint(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let (&byte, rest) = bytes.split_first()?;
        *bytes = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn write_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn sanitize_message(mut bytes: &[u8], schema: MessageSchema) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len());
    while !bytes.is_empty() {
        let key = read_varint(&mut bytes)?;
        let field = key >> 3;
        let wire_type = (key & 0x07) as u8;
        if field == 0 {
            return None;
        }

        if wire_type == 7 {
            let normal_wire_type = schema.field_wire_type(field)?;
            write_varint((field << 3) | u64::from(normal_wire_type), &mut out);
            schema.write_default(field, normal_wire_type, &mut out)?;
            continue;
        }

        write_varint(key, &mut out);
        match wire_type {
            0 => write_varint(read_varint(&mut bytes)?, &mut out),
            1 => {
                let (value, rest) = bytes.split_at_checked(8)?;
                out.extend_from_slice(value);
                bytes = rest;
            }
            2 => {
                let length = usize::try_from(read_varint(&mut bytes)?).ok()?;
                let (value, rest) = bytes.split_at_checked(length)?;
                let value = schema.child(field).map_or_else(|| Some(value.to_vec()), |child| sanitize_message(value, child))?;
                write_varint(value.len() as u64, &mut out);
                out.extend_from_slice(&value);
                bytes = rest;
            }
            5 => {
                let (value, rest) = bytes.split_at_checked(4)?;
                out.extend_from_slice(value);
                bytes = rest;
            }
            _ => return None,
        }
    }
    Some(out)
}

// bugged in reference implementation?
fn decode_repeated<M>(baseline: &[M], payloads: &[prost::bytes::Bytes], schema: MessageSchema) -> Option<Vec<M>> where
    M: Message + Default + Clone,
{
    const MAX_DELTA_ENTRIES: usize = 256;

    let mut messages = baseline.to_vec();
    for payload in payloads {
        let mut bytes = payload.as_ref();
        if bytes.is_empty() { messages.clear(); continue; }

        let mut key = read_varint(&mut bytes)?;
        if key & 0x07 == 7 {
            let length = usize::try_from(key >> 3).ok()?;
            if length > MAX_DELTA_ENTRIES { return None; }
            messages.resize_with(length, M::default);
            if bytes.is_empty() { continue; }
            key = read_varint(&mut bytes)?;
        }

        loop {
            if key & 0x07 != 2 { return None; }
            let index = usize::try_from(key >> 3).ok()?;
            let message = messages.get_mut(index)?;
            let length = usize::try_from(read_varint(&mut bytes)?).ok()?;
            let (delta, rest) = bytes.split_at_checked(length)?;
            let delta = sanitize_message(delta, schema)?;
            message.merge(delta.as_slice()).ok()?;
            bytes = rest;
            if bytes.is_empty() { break; }
            key = read_varint(&mut bytes)?;
        }
    }
    Some(messages)
}

fn replace_if_some<T>(target: &mut Option<T>, value: Option<T>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}

fn merge_buttons(target: &mut Option<CInButtonStatePb>, delta: CInButtonStatePb) {
    let target = target.get_or_insert_default();
    replace_if_some(&mut target.buttonstate1, delta.buttonstate1);
    replace_if_some(&mut target.buttonstate2, delta.buttonstate2);
    replace_if_some(&mut target.buttonstate3, delta.buttonstate3);
}

fn merge_qangle(target: &mut Option<CMsgQAngle>, delta: CMsgQAngle) {
    let target = target.get_or_insert_default();
    replace_if_some(&mut target.x, delta.x);
    replace_if_some(&mut target.y, delta.y);
    replace_if_some(&mut target.z, delta.z);
}

fn apply_delta(baseline: &CsgoUserCmdPb, delta_data: &[u8]) -> Option<CsgoUserCmdPb> {
    let sanitized = sanitize_message(delta_data, MessageSchema::CsgoUserCmd)?;
    let delta = DeltaCsgoUserCmdPb::decode(sanitized.as_slice()).ok()?;
    let mut next = baseline.clone();

    if !delta.input_history_delta.is_empty() {
        next.input_history = decode_repeated(
            &next.input_history,
            &delta.input_history_delta,
            MessageSchema::InputHistory,
        )?;
    }
    replace_if_some(&mut next.attack1_start_history_index, delta.attack1_start_history_index);
    replace_if_some(&mut next.attack2_start_history_index, delta.attack2_start_history_index);
    replace_if_some(&mut next.left_hand_desired, delta.left_hand_desired);
    replace_if_some(&mut next.is_predicting_body_shot_fx, delta.is_predicting_body_shot_fx);
    replace_if_some(&mut next.is_predicting_head_shot_fx, delta.is_predicting_head_shot_fx);
    replace_if_some(&mut next.is_predicting_kill_ragdolls, delta.is_predicting_kill_ragdolls);

    if let Some(delta_base) = delta.base {
        let base = next.base.get_or_insert_default();
        replace_if_some(&mut base.legacy_command_number, delta_base.legacy_command_number);
        replace_if_some(&mut base.client_tick, delta_base.client_tick);
        replace_if_some(&mut base.prediction_offset_ticks_x256, delta_base.prediction_offset_ticks_x256);
        if let Some(buttons) = delta_base.buttons_pb {
            merge_buttons(&mut base.buttons_pb, buttons);
        }
        if let Some(viewangles) = delta_base.viewangles {
            merge_qangle(&mut base.viewangles, viewangles);
        }
        replace_if_some(&mut base.forwardmove, delta_base.forwardmove);
        replace_if_some(&mut base.leftmove, delta_base.leftmove);
        replace_if_some(&mut base.upmove, delta_base.upmove);
        replace_if_some(&mut base.impulse, delta_base.impulse);
        replace_if_some(&mut base.weaponselect, delta_base.weaponselect);
        replace_if_some(&mut base.random_seed, delta_base.random_seed);
        replace_if_some(&mut base.mousedx, delta_base.mousedx);
        replace_if_some(&mut base.mousedy, delta_base.mousedy);
        replace_if_some(&mut base.pawn_entity_handle, delta_base.pawn_entity_handle);
        replace_if_some(&mut base.move_crc, delta_base.move_crc);
        replace_if_some(&mut base.consumed_server_angle_changes, delta_base.consumed_server_angle_changes);
        replace_if_some(&mut base.cmd_flags, delta_base.cmd_flags);
        if let Some(notes) = delta_base.execution_notes {
            let delta_notes = CBaseUserCmdExecutionNotes::decode(notes).ok()?;
            let notes = base.execution_notes.get_or_insert_default();
            replace_if_some(&mut notes.ignored_reason, delta_notes.ignored_reason);
        }
        if !delta_base.subtick_moves_delta.is_empty() {
            base.subtick_moves = decode_repeated(
                &base.subtick_moves,
                &delta_base.subtick_moves_delta,
                MessageSchema::SubtickMove,
            )?;
        }
    }

    Some(next)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserCmdReconstructionError {
    MissingPlayerSlot,
    InvalidPlayerSlot(i32),
    MissingCommandNumber {
        player_slot: i32,
    },
    MissingPayload {
        player_slot: i32,
        command_number: i32,
    },
    MissingBaseline {
        player_slot: i32,
        command_number: i32,
    },
    InvalidFullData {
        player_slot: i32,
        command_number: i32,
    },
    InvalidDelta {
        player_slot: i32,
        command_number: i32,
    },
    StaleCommand {
        player_slot: i32,
        command_number: i32,
        baseline_command_number: i32,
    },
}

impl UserCmdReconstructionError {
    #[must_use]
    pub(crate) fn is_missing_baseline(self) -> bool {
        matches!(self, Self::MissingBaseline { .. })
    }
}

impl fmt::Display for UserCmdReconstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for UserCmdReconstructionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCmdPayloadKind {
    Full,
    Delta,
    FullAndDelta,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconstructedUserCmd {
    pub player_slot: i32,
    pub command_number: i32,
    pub command: CsgoUserCmdPb,
    pub payload_kind: UserCmdPayloadKind,
}

/// Complete user commands reconstructed from one ordinary `svc_UserCmds` message.
///
/// Commands copied from full-packet checkpoints update the internal baseline but
/// do not emit this event.
#[derive(Debug, Clone, PartialEq)]
pub struct UserCommandsEvent {
    pub commands: Vec<ReconstructedUserCmd>,
}

impl crate::event::Event for UserCommandsEvent {}

/// Aggregate failures from user-command reconstruction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UserCmdDiagnostics {
    pub invalid_delta_count: u64,
}

#[derive(Debug, Clone)]
struct UserCmdBaseline {
    command_number: i32,
    command: CsgoUserCmdPb,
}

#[derive(Debug, Default)]
pub(crate) struct UserCmdReconstructor {
    baselines: HashMap<i32, UserCmdBaseline>,
}

impl UserCmdReconstructor {
    pub(crate) fn reconstruct(&mut self, envelope: &CMsgServerUserCmd) -> Result<ReconstructedUserCmd, UserCmdReconstructionError> {
        let player_slot = envelope.player_slot.ok_or(UserCmdReconstructionError::MissingPlayerSlot)?;
        if player_slot < 0 {
            return Err(UserCmdReconstructionError::InvalidPlayerSlot(player_slot));
        }
        let command_number = envelope.cmd_number.ok_or(UserCmdReconstructionError::MissingCommandNumber { player_slot })?;

        if envelope.data.is_none()
            && let Some(baseline) = self.baselines.get(&player_slot)
            && command_number <= baseline.command_number
        {
            return Err(UserCmdReconstructionError::StaleCommand { player_slot, command_number, baseline_command_number: baseline.command_number, });
        }

        if envelope.data.is_none() && envelope.delta_data.is_none() {
            self.baselines.remove(&player_slot);
            return Err(UserCmdReconstructionError::MissingPayload { player_slot, command_number });
        }

        let (mut command, payload_kind) = if let Some(data) = &envelope.data {
            let command = match CsgoUserCmdPb::decode(data.as_ref()) {
                Ok(command) => command,
                Err(_) => {
                    self.baselines.remove(&player_slot);
                    return Err(UserCmdReconstructionError::InvalidFullData { player_slot, command_number });
                }
            };
            let kind = if envelope.delta_data.is_some() { UserCmdPayloadKind::FullAndDelta } else { UserCmdPayloadKind::Full };
            (command, kind)
        } else {
            let baseline = self.baselines.get(&player_slot).ok_or(UserCmdReconstructionError::MissingBaseline { player_slot, command_number })?;
            (baseline.command.clone(), UserCmdPayloadKind::Delta)
        };

        if let Some(delta_data) = &envelope.delta_data {
            command = match apply_delta(&command, delta_data) {
                Some(command) => command,
                None => {
                    self.baselines.remove(&player_slot);
                    return Err(UserCmdReconstructionError::InvalidDelta { player_slot, command_number });
                }
            };
        }

        self.baselines.insert(player_slot, UserCmdBaseline { command_number, command: command.clone() });

        Ok(ReconstructedUserCmd { player_slot, command_number, command, payload_kind })
    }

    #[cfg(test)]
    #[must_use]
    fn baseline(&self, player_slot: i32) -> Option<(i32, &CsgoUserCmdPb)> {
        self.baselines.get(&player_slot).map(|baseline| (baseline.command_number, &baseline.command))
    }
}

// LLM code
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protobuf::{
        CBaseUserCmdPb, CSubtickMoveStep, CsgoInputHistoryEntryPb,
    };

    #[test]
    fn merges_usercmd_fields_and_repeated_subticks() {
        let bytes = [
            0x0a, 0x40, 0x10, 0xa5, 0x54, 0x1a, 0x06, 0x08, 0x90, 0x08, 0x10, 0x80,
            0x08, 0x22, 0x0a, 0x0d, 0x87, 0x85, 0x29, 0x40, 0x15, 0x36, 0x07, 0xc7,
            0x42, 0x35, 0x00, 0x00, 0x80, 0xbf, 0x50, 0xf8, 0xfb, 0xa7, 0xf7, 0x07,
            0x58, 0x51, 0x60, 0x06, 0x92, 0x01, 0x17, 0x0f, 0x02, 0x14, 0x08, 0x80,
            0x08, 0x10, 0x01, 0x1d, 0x00, 0x00, 0xd8, 0x3e, 0x45, 0x3c, 0x4e, 0x11,
            0xbf, 0x4d, 0xf0, 0x6a, 0xd5, 0x40,
        ];
        let mut baseline = CsgoUserCmdPb {
            base: Some(CBaseUserCmdPb {
                forwardmove: Some(0.75),
                viewangles: Some(CMsgQAngle {
                    z: Some(17.0),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let command = apply_delta(&baseline, &bytes).unwrap();
        let base = command.base.unwrap();
        let buttons = base.buttons_pb.unwrap();
        assert_eq!(buttons.buttonstate1, Some(0x410));
        assert_eq!(buttons.buttonstate2, Some(0x400));
        assert_eq!(base.forwardmove, Some(0.75));
        assert_eq!(base.leftmove, Some(-1.0));
        assert_eq!(base.viewangles.unwrap().z, Some(17.0));
        assert_eq!(base.subtick_moves.len(), 1);
        assert_eq!(base.subtick_moves[0].button(), 0x400);
        assert!(base.subtick_moves[0].pressed());
        assert!((base.subtick_moves[0].when() - 0.421875).abs() < f32::EPSILON);

        baseline.base.as_mut().unwrap().forwardmove = Some(2.0);
        assert_eq!(base.forwardmove, Some(0.75));
    }

    #[test]
    fn decodes_four_entry_history_resize() {
        let delta = [
            0x0a, 0x0a, 0x10, 0xd8, 0xa3, 0x04, 0x50, 0x88, 0xab, 0xd2, 0xa6, 0x02,
            0x12, 0x62, 0x27, 0x02, 0x12, 0x20, 0xd7, 0xa3, 0x04, 0x2d, 0xa0, 0x21,
            0xa9, 0x3e, 0x30, 0xd9, 0xa3, 0x04, 0x3d, 0x5b, 0x22, 0x26, 0x3d, 0x0a,
            0x12, 0x20, 0xd7, 0xa3, 0x04, 0x2d, 0xfc, 0x0d, 0xfb, 0x3e, 0x30, 0xd9,
            0xa3, 0x04, 0x3d, 0x56, 0x61, 0x4d, 0x3e, 0x12, 0x12, 0x20, 0xd7, 0xa3,
            0x04, 0x2d, 0xea, 0x7b, 0x26, 0x3f, 0x30, 0xd9, 0xa3, 0x04, 0x3d, 0x87,
            0x9a, 0xb8, 0x3e, 0x1a, 0x23, 0x12, 0x0f, 0x0d, 0xbe, 0x2a, 0xd8, 0x40,
            0x15, 0xda, 0x2d, 0xd6, 0xc0, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x20, 0xd7,
            0xa3, 0x04, 0x2d, 0xac, 0x71, 0x4f, 0x3f, 0x30, 0xd9, 0xa3, 0x04, 0x3d,
            0x08, 0x43, 0x05, 0x3f,
        ];

        let command = apply_delta(&CsgoUserCmdPb::default(), &delta).unwrap();
        let base = command.base.unwrap();
        assert_eq!(base.client_tick, Some(70_104));
        assert_eq!(base.random_seed, Some(617_911_688));
        assert_eq!(command.input_history.len(), 4);
        assert_eq!(command.input_history[0].render_tick_count, Some(70_103));
        assert_eq!(command.input_history[3].player_tick_count, Some(70_105));
        let angles = command.input_history[3].view_angles.as_ref().unwrap();
        assert!((angles.x.unwrap() - 6.755_217_6).abs() < f32::EPSILON);
        assert!((angles.y.unwrap() - -6.693_097).abs() < f32::EPSILON);
    }

    #[test]
    fn sparse_repeated_deltas_merge_into_existing_elements() {
        let baseline = vec![
            CsgoInputHistoryEntryPb {
                render_tick_count: Some(10),
                ..Default::default()
            },
            CsgoInputHistoryEntryPb {
                render_tick_count: Some(20),
                view_angles: Some(CMsgQAngle {
                    x: Some(1.0),
                    y: Some(2.0),
                    z: Some(3.0),
                }),
                ..Default::default()
            },
            CsgoInputHistoryEntryPb {
                render_tick_count: Some(30),
                player_tick_count: Some(31),
                ..Default::default()
            },
        ];
        let updates = [
            (
                0_u64,
                CsgoInputHistoryEntryPb {
                    render_tick_fraction: Some(0.25),
                    ..Default::default()
                },
            ),
            (
                2,
                CsgoInputHistoryEntryPb {
                    player_tick_count: Some(32),
                    ..Default::default()
                },
            ),
        ];
        let mut payload = Vec::new();
        for (index, update) in updates {
            write_varint((index << 3) | 2, &mut payload);
            let update = update.encode_to_vec();
            write_varint(update.len() as u64, &mut payload);
            payload.extend_from_slice(&update);
        }

        let decoded = decode_repeated(
            &baseline,
            &[payload.into()],
            MessageSchema::InputHistory,
        )
        .unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].render_tick_count, Some(10));
        assert_eq!(decoded[0].render_tick_fraction, Some(0.25));
        assert_eq!(decoded[1], baseline[1]);
        assert_eq!(decoded[2].render_tick_count, Some(30));
        assert_eq!(decoded[2].player_tick_count, Some(32));
    }

    #[test]
    fn repeated_element_resets_preserve_omitted_fields() {
        let baseline = vec![CSubtickMoveStep {
            button: Some(1),
            pressed: Some(true),
            when: Some(0.5),
            pitch_delta: Some(3.0),
            ..Default::default()
        }];
        let payload = prost::bytes::Bytes::from_static(&[0x02, 0x01, 0x17]);

        let decoded =
            decode_repeated(&baseline, &[payload], MessageSchema::SubtickMove).unwrap();
        assert_eq!(decoded[0].button, Some(1));
        assert_eq!(decoded[0].pressed, Some(false));
        assert_eq!(decoded[0].when, Some(0.5));
        assert_eq!(decoded[0].pitch_delta, Some(3.0));
    }

    #[test]
    fn repeated_resize_is_bounded_and_transactional() {
        let baseline = vec![CsgoInputHistoryEntryPb {
            render_tick_count: Some(10),
            ..Default::default()
        }];

        assert_eq!(
            decode_repeated(
                &baseline,
                &[prost::bytes::Bytes::from_static(&[0x07])],
                MessageSchema::InputHistory,
            ),
            Some(Vec::new())
        );
        assert!(decode_repeated(
            &baseline,
            &[prost::bytes::Bytes::from_static(&[0x8f, 0x10])],
            MessageSchema::InputHistory,
        )
        .is_none());
        assert_eq!(baseline[0].render_tick_count, Some(10));
    }

    #[test]
    fn wire_seven_uses_declared_nonzero_defaults() {
        let baseline = CsgoUserCmdPb {
            attack1_start_history_index: Some(4),
            base: Some(CBaseUserCmdPb {
                pawn_entity_handle: Some(123),
                ..Default::default()
            }),
            ..Default::default()
        };

        let command = apply_delta(&baseline, &[0x37, 0x0a, 0x01, 0x77]).unwrap();
        assert_eq!(command.attack1_start_history_index, Some(-1));
        assert_eq!(command.base.unwrap().pawn_entity_handle, Some(0x00ff_ffff));
    }

    #[test]
    fn reconstructor_requires_a_baseline_then_applies_deltas() {
        let mut reconstructor = UserCmdReconstructor::default();
        let delta = CMsgServerUserCmd {
            player_slot: Some(2),
            cmd_number: Some(11),
            delta_data: Some(vec![0x30, 0x00].into()),
            ..Default::default()
        };
        assert!(matches!(
            reconstructor.reconstruct(&delta),
            Err(UserCmdReconstructionError::MissingBaseline { .. })
        ));

        let full_command = CsgoUserCmdPb {
            attack1_start_history_index: Some(3),
            base: Some(CBaseUserCmdPb {
                forwardmove: Some(1.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let full = CMsgServerUserCmd {
            player_slot: Some(2),
            cmd_number: Some(10),
            data: Some(full_command.encode_to_vec().into()),
            ..Default::default()
        };
        let reconstructed = reconstructor.reconstruct(&full).unwrap();
        assert_eq!(reconstructed.payload_kind, UserCmdPayloadKind::Full);

        let reconstructed = reconstructor.reconstruct(&delta).unwrap();
        assert_eq!(reconstructed.payload_kind, UserCmdPayloadKind::Delta);
        assert_eq!(reconstructed.command.attack1_start_history_index, Some(0));
        assert_eq!(
            reconstructed.command.base.unwrap().forwardmove,
            Some(1.0)
        );
    }

    #[test]
    fn malformed_delta_invalidates_the_baseline() {
        let full_command = CsgoUserCmdPb {
            attack1_start_history_index: Some(3),
            ..Default::default()
        };
        let mut reconstructor = UserCmdReconstructor::default();
        reconstructor
            .reconstruct(&CMsgServerUserCmd {
                player_slot: Some(1),
                cmd_number: Some(10),
                data: Some(full_command.encode_to_vec().into()),
                ..Default::default()
            })
            .unwrap();

        let result = reconstructor.reconstruct(&CMsgServerUserCmd {
            player_slot: Some(1),
            cmd_number: Some(11),
            delta_data: Some(vec![0x0e].into()),
            ..Default::default()
        });
        assert!(matches!(
            result,
            Err(UserCmdReconstructionError::InvalidDelta { .. })
        ));
        assert_eq!(reconstructor.baseline(1), None);
        assert!(matches!(
            reconstructor.reconstruct(&CMsgServerUserCmd {
                player_slot: Some(1),
                cmd_number: Some(12),
                delta_data: Some(vec![0x30, 0x00].into()),
                ..Default::default()
            }),
            Err(UserCmdReconstructionError::MissingBaseline { .. })
        ));
    }

    #[test]
    fn full_payload_starts_a_new_command_sequence() {
        let mut reconstructor = UserCmdReconstructor::default();
        for command_number in [10, 1] {
            let command = CsgoUserCmdPb {
                attack1_start_history_index: Some(command_number),
                ..Default::default()
            };
            reconstructor
                .reconstruct(&CMsgServerUserCmd {
                    player_slot: Some(1),
                    cmd_number: Some(command_number),
                    data: Some(command.encode_to_vec().into()),
                    ..Default::default()
                })
                .unwrap();
        }

        let (command_number, command) = reconstructor.baseline(1).unwrap();
        assert_eq!(command_number, 1);
        assert_eq!(command.attack1_start_history_index, Some(1));
    }

}
