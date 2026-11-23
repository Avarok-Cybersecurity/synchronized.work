//! The test theme (a high-contrast palette) and configuration: every value chosen for the
//! tests, none a default production code could fall back to (PCND).

use sw_app_core::fit::FitParams;
use sw_app_core::measure::MeasureParams;
use sw_app_core::notify_retry::NotifyRetry;
use sw_app_core::pace::HostCallBudget;
use sw_app_core::*;
use sw_graph::{Motion, Spring, Theme};
use sw_view::{KindSizes, ViewParams};

pub fn theme(motion: Motion) -> AppTheme {
    AppTheme {
        graph: Theme {
            background: 0xFFFF_FFFF,
            edge: 0x6060_60FF,
            edge_width: 1.5,
            project: 0x2040_80FF,
            task: 0x3060_A0FF,
            person: 0x4080_40FF,
            agent: 0x8040_80FF,
            progress: 0xA060_20FF,
            label: 0x1010_10FF,
            label_size: 12.0,
        },
        view: ViewParams {
            node_gap: 16.0,
            level_gap: 72.0,
            sizes: KindSizes {
                project: (120.0, 32.0),
                task: (110.0, 28.0),
                person: (28.0, 28.0),
                agent: (24.0, 24.0),
                progress: (100.0, 22.0),
            },
        },
        margin: 16.0,
        text: 0x1010_10FF,
        muted: 0x4040_40FF,
        error: 0x9010_10FF,
        button: 0x1040_A0FF,
        button_text: 0xFFFF_FFFF,
        field: 0xF0F0_F0FF,
        title_px: 20.0,
        body_px: 14.0,
        line_height: 20.0,
        button_size: (220.0, 36.0),
        motion,
    }
}

pub const FULL: Motion = Motion::Full(Spring {
    stiffness: 170.0,
    damping: 26.0,
    rest: 0.5,
});

pub fn config() -> AppConfig {
    AppConfig {
        server: "synchronized.work".into(),
        site_url: "https://synchronized.work".into(),
        client_build: "sw-app@test".into(),
        session: SessionLimits {
            max_msg_bytes: 65536,
            max_msgs_per_s: 8,
        },
        poll: PollConfig {
            origin: "https://synchronized.work".into(),
            max_ms: 25000,
        },
        notify_memory: 4,
        notify_retry: NotifyRetry {
            queue: 4,
            attempts: 3,
            spacing_ms: 250.0,
        },
        rect_only: RectOnlyParams {
            arc_segments: 4,
            path_line_width: 1.0,
        },
        // These tests feed few or no frames, so the App's clock barely moves: a budget no
        // test reaches keeps them about what they test. The budget itself is tested with its
        // own values (`budget.rs`, and the paced story in sw-node).
        host_calls: HostCallBudget {
            per_s: 1_000_000,
            reserve: 16,
        },
        measure: MeasureParams {
            capacity: 512,
            estimate_em: 0.5,
        },
        fit: FitParams {
            label_min_px: 10.0,
            label_descent_em: 0.25,
            region_min: (24.0, 24.0),
        },
    }
}
