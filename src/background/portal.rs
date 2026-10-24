//! Native Background portal transport shared by startup and close requests.
//! Every request preserves automatic startup instead of silently removing its
//! desktop registration when a window closes. Requests are coalesced in flight.
use super::{INTERFACE, PATH, PORTAL};
use adw::{
    gtk::{gio, glib},
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Grant {
    pub background: bool,
    pub autostart: bool,
}
type Reply = Rc<Cell<Option<Grant>>>;
thread_local! { static WAITERS: RefCell<Option<Vec<Reply>>> = const { RefCell::new(None) }; }

pub(super) async fn request() -> Grant {
    let reply = Rc::new(Cell::new(None));
    let leader = WAITERS.with(|waiters| {
        let mut waiters = waiters.borrow_mut();
        if let Some(waiters) = waiters.as_mut() {
            waiters.push(reply.clone());
            false
        } else {
            *waiters = Some(vec![reply.clone()]);
            true
        }
    });
    if leader {
        let grant = request_portal().await;
        WAITERS.with(|waiters| {
            for reply in waiters.borrow_mut().take().unwrap_or_default() {
                reply.set(Some(grant));
            }
        });
    }
    loop {
        if let Some(grant) = reply.get() {
            return grant;
        }
        glib::timeout_future(Duration::from_millis(50)).await;
    }
}

fn options(token: &str) -> HashMap<&str, glib::Variant> {
    HashMap::from([
        ("handle_token", token.to_variant()),
        (
            "reason",
            "Start DriveSync at login and continue synchronizing when its window is closed"
                .to_variant(),
        ),
        ("autostart", true.to_variant()),
        (
            "commandline",
            vec!["drivesync", "--background"].to_variant(),
        ),
        ("dbus-activatable", false.to_variant()),
    ])
}

/// Transport errors, cancellation and malformed responses fail closed. Background
/// permission does not imply that the desktop enabled automatic session startup.
fn decode(value: &glib::Variant) -> Grant {
    let Some((0, values)) = value.get::<(u32, HashMap<String, glib::Variant>)>() else {
        return Grant::default();
    };
    let background = values.get("background").and_then(|v| v.get::<bool>()) == Some(true);
    Grant {
        background,
        autostart: background
            && values.get("autostart").and_then(|v| v.get::<bool>()) == Some(true),
    }
}

/// Requests permission and waits for the portal response, subscribing before
/// sending the request to avoid a fast-response race. Timeout closes the request.
async fn request_portal() -> Grant {
    let Ok(bus) = gio::bus_get_future(gio::BusType::Session).await else {
        return Grant::default();
    };
    let Some(sender) = bus.unique_name() else {
        return Grant::default();
    };
    let token = format!("drivesync{}", glib::monotonic_time());
    let handle = format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        sender.trim_start_matches(':').replace('.', "_"),
        token
    );
    let response = Rc::new(RefCell::new(None));
    let received = response.clone();
    let _subscription = bus.subscribe_to_signal(
        Some(PORTAL),
        Some("org.freedesktop.portal.Request"),
        Some("Response"),
        Some(&handle),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            *received.borrow_mut() = Some(decode(signal.parameters));
        },
    );
    let options = options(&token);
    if bus
        .call_future(
            Some(PORTAL),
            PATH,
            INTERFACE,
            "RequestBackground",
            Some(&("", options).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            10_000,
        )
        .await
        .is_err()
    {
        return Grant::default();
    }
    for _ in 0..600 {
        if let Some(allowed) = *response.borrow() {
            return allowed;
        }
        glib::timeout_future(Duration::from_millis(100)).await;
    }
    let _ = bus
        .call_future(
            Some(PORTAL),
            &handle,
            "org.freedesktop.portal.Request",
            "Close",
            None,
            None,
            gio::DBusCallFlags::NONE,
            2_000,
        )
        .await;
    Grant::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grants_distinguish_background_from_actual_autostart_and_fail_closed() {
        for (code, background, autostart, expected) in [
            (
                0u32,
                true,
                true,
                Grant {
                    background: true,
                    autostart: true,
                },
            ),
            (
                0,
                true,
                false,
                Grant {
                    background: true,
                    autostart: false,
                },
            ),
            (0, false, true, Grant::default()),
            (1, true, true, Grant::default()),
            (2, true, true, Grant::default()),
        ] {
            let values = HashMap::from([
                ("background", background.to_variant()),
                ("autostart", autostart.to_variant()),
            ]);
            assert_eq!(decode(&(code, values).to_variant()), expected);
        }
        assert_eq!(decode(&"invalid".to_variant()), Grant::default());
        assert_eq!(
            decode(&(0u32, HashMap::<String, glib::Variant>::new()).to_variant()),
            Grant::default()
        );
    }
    #[test]
    fn every_request_preserves_hidden_autostart_with_an_argument_array() {
        let options = options("test_token");
        assert_eq!(options["autostart"].get::<bool>(), Some(true));
        assert_eq!(
            options["commandline"].get::<Vec<String>>(),
            Some(vec!["drivesync".into(), "--background".into()])
        );
        assert_eq!(options["dbus-activatable"].get::<bool>(), Some(false));
        assert_eq!(
            options["handle_token"].get::<String>().as_deref(),
            Some("test_token")
        );
    }
}
