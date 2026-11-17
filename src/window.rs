//! One startup activation request for the Linux X11 window.
//!
//! Miniquad raises its X11 window but exposes neither activation nor a native
//! window handle. A temporary per-process class identifies our own window,
//! including when another instance of the game is already running.

pub fn startup_class() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        static CLASS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        CLASS.get_or_init(|| format!("scorched-startup-{}", std::process::id()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        "scorched"
    }
}

pub fn activate_on_startup() {
    #[cfg(target_os = "linux")]
    if let Err(error) = x11::activate() {
        eprintln!("Could not request startup window focus: {error}");
    }
}

#[cfg(target_os = "linux")]
mod x11 {
    use x11rb::{
        connection::Connection,
        properties::WmHints,
        protocol::{
            xproto::{
                AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt, EventMask,
                InputFocus, MapState, PropMode,
            },
            Event,
        },
        wrapper::ConnectionExt as _,
        CURRENT_TIME,
    };

    type Error = Box<dyn std::error::Error>;

    pub(super) fn activate() -> Result<(), Error> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let marker = super::startup_class().as_bytes();
        let mut candidates = vec![(root, 0)];
        let mut own = None;
        // Reparenting WMs can put the client inside a frame. Bound the search
        // and match the unique class, never just the shared window title.
        for _ in 0..4096 {
            let Some((window, depth)) = candidates.pop() else {
                break;
            };
            if window != root {
                let reply = conn
                    .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 128)?
                    .reply();
                if let Ok(reply) = reply {
                    if reply.value.split(|&b| b == 0).any(|part| part == marker) {
                        own = Some(window);
                        break;
                    }
                }
            }
            if depth < 3 {
                if let Ok(reply) = conn.query_tree(window)?.reply() {
                    candidates.extend(reply.children.into_iter().map(|child| (child, depth + 1)));
                }
            }
        }
        let window = own.ok_or("the game's X11 window was not found")?;
        if conn.get_window_attributes(window)?.reply()?.map_state != MapState::VIEWABLE {
            return Err("the game's X11 window is not viewable".into());
        }

        conn.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"scorched\0ScorchedEarth\0",
        )?
        .check()?;
        let mut hints = WmHints::get(&conn, window)?.reply()?.unwrap_or_default();
        hints.input = Some(true);
        hints.set(&conn, window)?.check()?;

        // Get a server timestamp from our own PropertyNotify, without sleeps,
        // borrowing another application's user time, or using wall-clock time.
        let pid = atom(&conn, b"_NET_WM_PID")?;
        conn.change_window_attributes(
            window,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
        conn.change_property32(
            PropMode::REPLACE,
            window,
            pid,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )?
        .check()?;
        let mut timestamp = CURRENT_TIME;
        while let Some(event) = conn.poll_for_event()? {
            if let Event::PropertyNotify(event) = event {
                if event.window == window && event.atom == pid {
                    timestamp = event.time;
                    break;
                }
            }
        }

        let supported = atom(&conn, b"_NET_SUPPORTED")?;
        let active = atom(&conn, b"_NET_ACTIVE_WINDOW")?;
        let wm_supports_activation = conn
            .get_property(false, root, supported, AtomEnum::ATOM, 0, 4096)?
            .reply()?
            .value32()
            .is_some_and(|mut atoms| atoms.any(|atom| atom == active));
        if wm_supports_activation {
            // EWMH §3.8: application request, server timestamp, no currently
            // active window belonging to this client. The WM decides policy.
            let event = ClientMessageEvent::new(32, window, active, [1, timestamp, 0, 0, 0]);
            conn.send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            )?
            .check()?;
        } else {
            conn.set_input_focus(InputFocus::PARENT, window, timestamp)?
                .check()?;
        }
        conn.flush()?;
        Ok(())
    }

    fn atom(conn: &impl Connection, name: &[u8]) -> Result<u32, Error> {
        Ok(conn.intern_atom(false, name)?.reply()?.atom)
    }
}
