//! The app's icon set: simple 24-unit strokes drawn in the element's colour.

use rune::Icon;

macro_rules! stroke {
    ($name:ident, $id:literal, $body:literal) => {
        pub static $name: Icon = Icon {
            name: $id,
            view: 24.0,
            body: concat!(
                r#"<g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
                $body,
                "</g>"
            ),
        };
    };
}

macro_rules! solid {
    ($name:ident, $id:literal, $body:literal) => {
        pub static $name: Icon = Icon {
            name: $id,
            view: 24.0,
            body: concat!(r#"<g fill="currentColor" stroke="none">"#, $body, "</g>"),
        };
    };
}

stroke!(
    HOME,
    "vc-home",
    r#"<path d="M3 10.5 12 3l9 7.5"/><path d="M5 9.5V20h5v-6h4v6h5V9.5"/>"#
);
stroke!(
    MUSIC,
    "vc-music",
    r#"<path d="M9 18V5l11-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="17" cy="16" r="3"/>"#
);
stroke!(
    RADIO,
    "vc-radio",
    r#"<circle cx="12" cy="12" r="2"/><path d="M8.2 8.2a5.4 5.4 0 0 0 0 7.6M15.8 8.2a5.4 5.4 0 0 1 0 7.6M5.3 5.3a9.5 9.5 0 0 0 0 13.4M18.7 5.3a9.5 9.5 0 0 1 0 13.4"/>"#
);
stroke!(
    FILM,
    "vc-film",
    r#"<rect x="3" y="3" width="18" height="18" rx="2.5"/><path d="M7 3v18M17 3v18M3 8h4M3 12h18M3 16h4M17 8h4M17 16h4"/>"#
);
stroke!(
    DOWNLOAD,
    "vc-download",
    r#"<path d="M12 3v12M7 10l5 5 5-5"/><path d="M4 17v2.5A1.5 1.5 0 0 0 5.5 21h13a1.5 1.5 0 0 0 1.5-1.5V17"/>"#
);
stroke!(
    LIBRARY,
    "vc-library",
    r#"<rect x="3" y="4" width="4" height="16" rx="1"/><rect x="9" y="4" width="4" height="16" rx="1"/><path d="m15 5.2 3.9-1 2.6 15.6-3.9 1z"/>"#
);
stroke!(
    HISTORY,
    "vc-history",
    r#"<path d="M3 12a9 9 0 1 0 2.6-6.4L3 8"/><path d="M3 3v5h5M12 7v5l3.5 2"/>"#
);
stroke!(
    BOOKMARK,
    "vc-bookmark",
    r#"<path d="M6 3h12v18l-6-4.5L6 21z"/>"#
);
solid!(
    BOOKMARK_FILLED,
    "vc-bookmark-filled",
    r#"<path d="M6 2h12a1 1 0 0 1 1 1v18.5l-7-5.2-7 5.2V3a1 1 0 0 1 1-1z"/>"#
);
stroke!(
    QUEUE,
    "vc-queue",
    r#"<path d="M3 6h13M3 12h13M3 18h8"/><path d="m16 15 5 3-5 3z"/>"#
);
stroke!(
    SETTINGS,
    "vc-settings",
    r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.6 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.6-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>"#
);
stroke!(
    USER,
    "vc-user",
    r#"<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>"#
);
stroke!(
    SEARCH,
    "vc-search",
    r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-3.6-3.6"/>"#
);
solid!(
    PLAY,
    "vc-play",
    r#"<path d="M7 4.6v14.8a1 1 0 0 0 1.5.9l12-7.4a1 1 0 0 0 0-1.8l-12-7.4A1 1 0 0 0 7 4.6z"/>"#
);
solid!(
    PAUSE,
    "vc-pause",
    r#"<rect x="6" y="4" width="4.2" height="16" rx="1.2"/><rect x="13.8" y="4" width="4.2" height="16" rx="1.2"/>"#
);
solid!(
    NEXT,
    "vc-next",
    r#"<path d="M5 5.2v13.6a.9.9 0 0 0 1.4.8l9.6-6.8a1 1 0 0 0 0-1.6L6.4 4.4a.9.9 0 0 0-1.4.8z"/><rect x="17" y="4" width="2.6" height="16" rx="1"/>"#
);
solid!(
    PREV,
    "vc-prev",
    r#"<path d="M19 5.2v13.6a.9.9 0 0 1-1.4.8L8 12.8a1 1 0 0 1 0-1.6l9.6-6.8a.9.9 0 0 1 1.4.8z"/><rect x="4.4" y="4" width="2.6" height="16" rx="1"/>"#
);
stroke!(
    SHUFFLE,
    "vc-shuffle",
    r#"<path d="M16 3h5v5M4 20 21 3M21 16v5h-5M15 15l6 6M4 4l5 5"/>"#
);
stroke!(
    REPEAT,
    "vc-repeat",
    r#"<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14M7 22l-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/>"#
);
stroke!(
    REPEAT_ONE,
    "vc-repeat-one",
    r#"<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14M7 22l-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/><path d="M11 10h1v4"/>"#
);
stroke!(
    VOLUME,
    "vc-volume",
    r#"<path d="M11 5 6 9H2v6h4l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14"/>"#
);
stroke!(
    VOLUME_LOW,
    "vc-volume-low",
    r#"<path d="M11 5 6 9H2v6h4l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/>"#
);
stroke!(
    MUTE,
    "vc-mute",
    r#"<path d="M11 5 6 9H2v6h4l5 4z"/><path d="m22 9-6 6M16 9l6 6"/>"#
);
stroke!(
    FULLSCREEN,
    "vc-fullscreen",
    r#"<path d="M3 8V3h5M16 3h5v5M21 16v5h-5M8 21H3v-5"/>"#
);
stroke!(
    FULLSCREEN_EXIT,
    "vc-fullscreen-exit",
    r#"<path d="M8 3v5H3M21 8h-5V3M16 21v-5h5M3 16h5v5"/>"#
);
stroke!(
    THEATER,
    "vc-theater",
    r#"<rect x="2" y="5" width="20" height="14" rx="2"/>"#
);
stroke!(
    MINI,
    "vc-mini",
    r#"<rect x="2" y="4" width="20" height="16" rx="2"/><rect x="11" y="11" width="8" height="6" rx="1" fill="currentColor"/>"#
);
stroke!(
    CAPTIONS,
    "vc-captions",
    r#"<rect x="2" y="5" width="20" height="14" rx="2.5"/><path d="M10 10.5a2.2 2.2 0 1 0 0 3M17 10.5a2.2 2.2 0 1 0 0 3"/>"#
);
stroke!(PLUS, "vc-plus", r#"<path d="M12 5v14M5 12h14"/>"#);
stroke!(CHECK, "vc-check", r#"<path d="m4 12.5 5 5L20 6.5"/>"#);
stroke!(CLOSE, "vc-close", r#"<path d="M6 6l12 12M18 6 6 18"/>"#);
stroke!(BACK, "vc-back", r#"<path d="M15 5l-7 7 7 7"/>"#);
stroke!(FORWARD, "vc-forward", r#"<path d="m9 5 7 7-7 7"/>"#);
stroke!(DOWN, "vc-down", r#"<path d="m6 9 6 6 6-6"/>"#);
stroke!(UP, "vc-up", r#"<path d="m6 15 6-6 6 6"/>"#);
solid!(
    MORE,
    "vc-more",
    r#"<circle cx="12" cy="5" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="12" cy="19" r="2"/>"#
);
stroke!(
    LINK,
    "vc-link",
    r#"<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>"#
);
stroke!(
    LYRICS,
    "vc-lyrics",
    r#"<path d="M4 6h10M4 10h10M4 14h6"/><path d="M18 18V7l3-1"/><circle cx="16" cy="18" r="2"/>"#
);
stroke!(
    MOON,
    "vc-moon",
    r#"<path d="M20.5 14.5A8.5 8.5 0 1 1 9.5 3.5a7 7 0 0 0 11 11z"/>"#
);
stroke!(
    SPEAKER,
    "vc-speaker",
    r#"<rect x="5" y="2" width="14" height="20" rx="2.5"/><circle cx="12" cy="14" r="4"/><path d="M12 6h.01"/>"#
);
stroke!(
    SLIDERS,
    "vc-sliders",
    r#"<path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6"/>"#
);
stroke!(
    PALETTE,
    "vc-palette",
    r#"<path d="M12 22a10 10 0 1 1 10-10c0 3-2.5 4-4.5 4H16a2 2 0 0 0-1.5 3.3A1.6 1.6 0 0 1 12 22z"/><circle cx="7.5" cy="11" r="1.2"/><circle cx="10.5" cy="7" r="1.2"/><circle cx="15" cy="7.5" r="1.2"/>"#
);
stroke!(
    SPARKLES,
    "vc-sparkles",
    r#"<path d="M12 3l1.8 4.7L18.5 9.5 13.8 11.3 12 16l-1.8-4.7L5.5 9.5l4.7-1.8z"/><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"/>"#
);
stroke!(
    REFRESH,
    "vc-refresh",
    r#"<path d="M21 12a9 9 0 0 1-15.4 6.4L3 16"/><path d="M3 21v-5h5M3 12a9 9 0 0 1 15.4-6.4L21 8"/><path d="M21 3v5h-5"/>"#
);
stroke!(
    TRASH,
    "vc-trash",
    r#"<path d="M3 6h18M8 6V4h8v2M19 6l-1 15H6L5 6M10 11v6M14 11v6"/>"#
);
stroke!(
    FOLDER,
    "vc-folder",
    r#"<path d="M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z"/>"#
);
stroke!(
    EXTERNAL,
    "vc-external",
    r#"<path d="M14 3h7v7M10 14 21 3M19 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5"/>"#
);
stroke!(
    GLOBE,
    "vc-globe",
    r#"<circle cx="12" cy="12" r="9.5"/><path d="M2.5 12h19M12 2.5c2.6 2.6 3.9 5.8 3.9 9.5s-1.3 6.9-3.9 9.5c-2.6-2.6-3.9-5.8-3.9-9.5S9.4 5.1 12 2.5z"/>"#
);
stroke!(
    KEY,
    "vc-key",
    r#"<circle cx="7.5" cy="15.5" r="4.5"/><path d="M10.7 12.3 20 3M16 7l3 3M18 5l2 2"/>"#
);
stroke!(
    INFO,
    "vc-info",
    r#"<circle cx="12" cy="12" r="9.5"/><path d="M12 16.5V11M12 7.5h.01"/>"#
);
stroke!(
    WARNING,
    "vc-warning",
    r#"<path d="M10.3 3.9 1.9 18a2 2 0 0 0 1.7 3h16.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/>"#
);
stroke!(
    SIGNAL,
    "vc-signal",
    r#"<path d="M2 20h.01M7 20v-4M12 20v-8M17 20V8M22 4v16"/>"#
);
stroke!(
    COPY,
    "vc-copy",
    r#"<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/>"#
);
stroke!(
    LOGOUT,
    "vc-logout",
    r#"<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>"#
);
stroke!(
    GAUGE,
    "vc-gauge",
    r#"<path d="M12 14l4-4"/><path d="M3.3 19a10 10 0 1 1 17.4 0"/>"#
);
stroke!(
    PAUSE_OUTLINE,
    "vc-pause-outline",
    r#"<rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/>"#
);

/// The app mark, drawn in its own colours (same art as the Android launcher icon).
pub static LOGO: Icon = Icon {
    name: "vc-logo",
    view: 512.0,
    body: r##"<defs><linearGradient id="vcg" x1="106" y1="106" x2="406" y2="406" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#FFB547"/><stop offset="1" stop-color="#FF4D8D"/></linearGradient></defs><rect width="512" height="512" rx="116" fill="#14111A"/><path d="M370.9 159.6A150 150 0 1 0 370.9 352.4" fill="none" stroke="url(#vcg)" stroke-width="46" stroke-linecap="round"/><path d="M220 198L322 256L220 314Z" fill="#FFFFFF" stroke="#FFFFFF" stroke-width="26" stroke-linejoin="round"/>"##,
};
