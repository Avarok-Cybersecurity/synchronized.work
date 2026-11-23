//! The SDK's types to the core's and back, one function per crossing. Nothing here decides:
//! each function is a 1:1 translation, so the binding is exactly the core's behaviour.

use citadel_app_sdk_types::calls::{
    DrawCommand as SdkDraw, HttpMethod, HttpRequest, RegionRole as SdkRole, SessionEvent,
    SessionStatus, StreamChunk,
};
use citadel_app_sdk_types::{AppEvent, Bytes, PointerKind, Refused};
use sw_app_core::fallback::{self, HttpGet, PlanError};
use sw_app_core::{DrawCommand, HostSessionStatus, Input, RegionRole, Size};

pub fn region_role_of(role: RegionRole) -> SdkRole {
    match role {
        RegionRole::Button => SdkRole::Button,
        RegionRole::Link => SdkRole::Link,
        RegionRole::Img => SdkRole::Img,
        RegionRole::Group => SdkRole::Group,
    }
}

pub fn draw_command_of(command: DrawCommand) -> SdkDraw {
    match command {
        DrawCommand::Clear { color } => SdkDraw::Clear { color },
        DrawCommand::FillRect { x, y, w, h, color } => SdkDraw::FillRect { x, y, w, h, color },
        DrawCommand::StrokeRect {
            x,
            y,
            w,
            h,
            line_width,
            color,
        } => SdkDraw::StrokeRect {
            x,
            y,
            w,
            h,
            line_width,
            color,
        },
        DrawCommand::Line {
            x1,
            y1,
            x2,
            y2,
            line_width,
            color,
        } => SdkDraw::Line {
            x1,
            y1,
            x2,
            y2,
            line_width,
            color,
        },
        DrawCommand::FillText {
            x,
            y,
            text,
            px,
            color,
        } => SdkDraw::FillText {
            x,
            y,
            text,
            px,
            color,
        },
        DrawCommand::FillCircle { cx, cy, r, color } => SdkDraw::FillCircle { cx, cy, r, color },
        DrawCommand::StrokeCircle {
            cx,
            cy,
            r,
            line_width,
            color,
        } => SdkDraw::StrokeCircle {
            cx,
            cy,
            r,
            line_width,
            color,
        },
        DrawCommand::Path {
            points,
            closed,
            fill,
            stroke,
        } => SdkDraw::Path {
            points,
            closed,
            fill,
            stroke,
        },
        DrawCommand::Arc {
            cx,
            cy,
            r,
            start,
            end,
            line_width,
            color,
        } => SdkDraw::Arc {
            cx,
            cy,
            r,
            start,
            end,
            line_width,
            color,
        },
        DrawCommand::Region {
            id,
            x,
            y,
            w,
            h,
            label,
            role,
        } => SdkDraw::Region {
            id,
            x,
            y,
            w,
            h,
            label,
            role: region_role_of(role),
        },
    }
}

pub fn status_of(status: SessionStatus) -> HostSessionStatus {
    match status {
        SessionStatus::Connecting => HostSessionStatus::Connecting,
        SessionStatus::Connected => HostSessionStatus::Connected,
        SessionStatus::Closed { reason } => HostSessionStatus::Closed { reason },
    }
}

/// A refusal as the core reads a reason: the agent's own reason as it is (`no_identity` is
/// `Unavailable("no_identity")`, so it reaches `transport::after_open_failure` unchanged).
pub fn reason_of(refused: &Refused) -> String {
    match refused {
        Refused::Ungranted => "ungranted".to_owned(),
        Refused::Unavailable(reason) => reason.clone(),
        Refused::Limit(what) => format!("limit: {what}"),
        Refused::Policy(reason) => format!("policy: {reason}"),
        Refused::Remote(status) => format!("remote: {status}"),
    }
}

/// The input an event is for the core, if any. `Init` (the start) and `Frame` (`App::frame`)
/// have their own paths. Session events count only for the App's own session.
pub fn input_of(event: AppEvent, session: Option<u32>) -> Option<Input> {
    match event {
        AppEvent::Resize { width, height, .. } => Some(Input::Resize {
            size: Size {
                width: width as f32,
                height: height as f32,
            },
        }),
        AppEvent::Pointer(p) if p.kind == PointerKind::Up && p.button == 0 => {
            Some(Input::PointerUp { x: p.x, y: p.y })
        }
        AppEvent::Key(k) if k.down => Some(Input::Key { key: k.key }),
        AppEvent::Activate { id } => Some(Input::Activate { id }),
        AppEvent::Session { session: s, event } if Some(s) == session => Some(match event {
            SessionEvent::Message { body } => Input::SessionMessage { body: body.0 },
            SessionEvent::State { state } => Input::SessionState {
                status: status_of(state),
            },
        }),
        _ => None,
    }
}

/// A `CanvasDraw` answer: a refusal is the core's to retry (`None`: the host took the frame).
pub fn draw_reply<T>(reply: &Result<T, Refused>) -> Option<Input> {
    reply.is_err().then_some(Input::DrawRefused)
}

/// A `Notify` answer, for the notification of `event_id`.
pub fn notify_reply<T>(event_id: [u8; 32], reply: &Result<T, Refused>) -> Input {
    match reply {
        Ok(_) => Input::NotifyDelivered { event_id },
        Err(_) => Input::NotifyRefused { event_id },
    }
}

pub fn frame_input(dt_ms: u32) -> Input {
    Input::Frame {
        dt_ms: dt_ms as f32,
    }
}

/// The `HostFetchOpen` request for the core's `GET`. Its headers are checked again here, so no
/// `Upgrade`, `Connection` or `Sec-*` header can leave the App whatever the core became.
pub fn http_request_of(get: HttpGet) -> Result<HttpRequest, PlanError> {
    fallback::check_headers(&get.headers)?;
    Ok(HttpRequest {
        method: HttpMethod::Get,
        url: get.url,
        headers: get.headers,
        body: Bytes(Vec::new()),
    })
}

/// Where a fallback response stands after one `NetStreamRead`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStep {
    /// Read again.
    More,
    /// The body is complete: close the stream and hand the body to the core.
    End,
    /// The body passed `max` bytes: close the stream and report the failure.
    TooLarge,
}

pub fn read_step(body: &mut Vec<u8>, chunk: StreamChunk, max: usize) -> StreamStep {
    if body.len().saturating_add(chunk.bytes.0.len()) > max {
        return StreamStep::TooLarge;
    }
    body.extend(chunk.bytes.0);
    if chunk.end {
        StreamStep::End
    } else {
        StreamStep::More
    }
}
